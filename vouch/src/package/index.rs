use anyhow::Result;
use std::collections::HashSet;

use super::common;
use crate::common::StoreTransaction;
use crate::registry;

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub package_name: Option<&'a str>,
    pub package_version: Option<&'a str>,

    // Filters match for any in set.
    pub registry_host_names: Option<std::collections::BTreeSet<&'a str>>,
}

pub fn setup(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        r"
        CREATE TABLE IF NOT EXISTS package (
            id                         INTEGER NOT NULL PRIMARY KEY,
            name                       TEXT NOT NULL,
            version                    TEXT NOT NULL,
            registry_ids               BLOB NOT NULL,
            artifact_hash              TEXT NOT NULL,

            UNIQUE(name, version, artifact_hash)
        )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(
    package_name: &str,
    package_version: &str,
    registries: &std::collections::BTreeSet<registry::Registry>,
    artifact_hash: &str,
    tx: &StoreTransaction,
) -> Result<common::Package> {
    assert!(
        !registries.is_empty(),
        "At least one registry must be assigned to a package before index insert."
    );
    let registry_ids: Vec<crate::common::index::ID> =
        registries.into_iter().map(|c| c.id).collect();
    let registry_ids = bincode::serialize(&registry_ids)?;

    tx.index_tx().execute_named(
        r"
            INSERT INTO package (
                name,
                version,
                registry_ids,
                artifact_hash
            )
            VALUES (
                :name,
                :version,
                :registry_ids,
                :artifact_hash
            )
        ",
        rusqlite::named_params! {
            ":name": package_name,
            ":version": package_version,
            ":registry_ids": registry_ids,
            ":artifact_hash": artifact_hash,
        },
    )?;
    Ok(common::Package {
        id: tx.index_tx().last_insert_rowid(),
        name: package_name.to_string(),
        version: package_version.to_string(),
        registries: registries.clone(),
        artifact_hash: artifact_hash.to_string(),
    })
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<common::Package>> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let package_name = crate::common::index::get_like_clause_param(fields.package_name);
    let package_version = crate::common::index::get_like_clause_param(fields.package_version);

    let mut statement = tx.index_tx().prepare(
        r"
            SELECT *
            FROM package
            WHERE
                package.id LIKE :package_id ESCAPE '\'
                AND name LIKE :name ESCAPE '\'
                AND version LIKE :version ESCAPE '\'
        ",
    )?;
    let mut rows = statement.query_named(&[
        (":package_id", &id),
        (":name", &package_name),
        (":version", &package_version),
    ])?;

    let mut packages = HashSet::new();
    while let Some(row) = rows.next()? {
        let registry_ids: Option<Result<Vec<crate::common::index::ID>>> = row
            .get::<_, Option<Vec<u8>>>(3)?
            .map(|x| Ok(bincode::deserialize(&x)?));
        let registries = match registry_ids {
            Some(registry_ids) => {
                let registry_ids = registry_ids?;
                registry::index::get(
                    &registry::index::Fields {
                        ids: Some(&registry_ids),
                        ..Default::default()
                    },
                    &tx,
                )?
                .into_iter()
                .collect()
            }
            None => std::collections::BTreeSet::<registry::Registry>::new(),
        };

        // Skip package if none of the given registry host names match to any registry.
        if let Some(registry_host_names) = &fields.registry_host_names {
            let mut found_match = false;
            for registry_host_name in registry_host_names {
                found_match |= registries
                    .iter()
                    .any(|registry| &registry.host_name.as_str() == registry_host_name);
            }
            if !found_match {
                continue;
            }
        }

        let package = common::Package {
            id: row.get(0)?,
            name: row.get(1)?,
            version: row.get(2)?,
            registries: registries,
            artifact_hash: row.get(4)?,
        };
        packages.insert(package);
    }
    Ok(packages)
}

/// Merge packages from incoming index into another index. Returns the newly merged packages.
pub fn merge(
    incoming_tx: &StoreTransaction,
    tx: &StoreTransaction,
) -> Result<HashSet<common::Package>> {
    let incoming_packages = get(&Fields::default(), &incoming_tx)?;
    let existing_packages = get(&Fields::default(), &tx)?;

    let mut new_packages = HashSet::new();
    for package in
        crate::common::index::get_difference_sans_id(&incoming_packages, &existing_packages)?
    {
        let mut new_registries = std::collections::BTreeSet::new();
        for registry in package.registries {
            let new_registry = registry::index::ensure(
                &registry.host_name,
                &registry.human_url,
                &registry.artifact_url,
                &tx,
            )?;
            new_registries.insert(new_registry);
        }

        let package = insert(
            &package.name,
            &package.version,
            &new_registries.clone(),
            &package.artifact_hash,
            &tx,
        )?;
        new_packages.insert(package);
    }
    Ok(new_packages)
}

pub fn remove(fields: &Fields, tx: &StoreTransaction) -> Result<()> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    tx.index_tx().execute_named(
        r"
        DELETE
        FROM package
        WHERE
            id LIKE :id ESCAPE '\'
    ",
        &[(":id", &id)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{format_err, Result};

    #[test]
    fn test_merge_correct_difference_set() -> Result<()> {
        let registries = maplit::btreeset! { registry::Registry {
            id: 2,
            host_name: "pypi.org".to_string(),
            human_url: url::Url::parse( "https://pypi.org/pypi/py-cpuinfo/5.0.0/")?,
            artifact_url: url::Url::parse("https://files.pythonhosted.org/packages/42/60/63f28a5401da733043abe7053e7d9591491b4784c4f87c339bf51215aa0a/py-cpuinfo-5.0.0.tar.gz")?,
        }};

        let existing_packages = maplit::hashset! {
            common::Package {
                id: 2,
                name: "py-cpuinfo".to_string(),
                version: "5.0.0".to_string(),
                registries: registries.clone(),
                artifact_hash: "4a42aafca3d68e4feee71fde2779c6b30be37370aa6deb3e88356bbec266d017".to_string()
            }
        };
        let incoming_packages = maplit::hashset! {
            common::Package {
                id: 3,
                name: "py-cpuinfo".to_string(),
                version: "5.0.0".to_string(),
                registries: registries.clone(),
                artifact_hash: "4a42aafca3d68e4feee71fde2779c6b30be37370aa6deb3e88356bbec266d017".to_string()
            }
        };
        let result =
            crate::common::index::get_difference_sans_id(&incoming_packages, &existing_packages)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn test_get_on_registry_host_names() -> Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        crate::store::index::setup(&tx)?;

        let registries_1 = maplit::btreeset! { registry::Registry {
            id: 1,
            host_name: "host_name_1".to_string(),
            human_url: url::Url::parse( "https://pypi.org/pypi/py-cpuinfo/5.0.0/")?,
            artifact_url: url::Url::parse("https://artifact_url_1.com")?,
        }};
        let package_1 = common::Package {
            id: 1,
            name: "py-cpuinfo".to_string(),
            version: "5.0.0".to_string(),
            registries: registries_1.clone(),
            artifact_hash: "artifact_hash_1".to_string(),
        };

        let registries_2 = maplit::btreeset! { registry::Registry {
            id: 2,
            host_name: "host_name_2".to_string(),
            human_url: url::Url::parse( "https://pypi.org/pypi/py-cpuinfo/5.0.0/")?,
            artifact_url: url::Url::parse("https://artifact_url_2.com")?,
        }};
        let package_2 = common::Package {
            id: 2,
            name: "py-cpuinfo".to_string(),
            version: "5.0.0".to_string(),
            registries: registries_2.clone(),
            artifact_hash: "artifact_hash_2".to_string(),
        };

        for package in vec![package_1, package_2] {
            let mut registries = std::collections::BTreeSet::<registry::Registry>::new();
            for registry in package.registries {
                registries.insert(registry::index::ensure(
                    &registry.host_name,
                    &registry.human_url,
                    &registry.artifact_url,
                    &tx,
                )?);
            }
            insert(
                &package.name,
                &package.version,
                &registries,
                &package.artifact_hash,
                &tx,
            )?;
        }

        let result = get(
            &Fields {
                registry_host_names: Some(maplit::btreeset! {"host_name_1"}),
                ..Default::default()
            },
            &tx,
        )?;
        assert!(result.len() == 1);
        let result = &result
            .iter()
            .next()
            .ok_or(format_err!("Failed to retrieve any packages."))?
            .artifact_hash;
        let expected = "artifact_hash_1";
        assert_eq!(result, expected);
        Ok(())
    }
}
