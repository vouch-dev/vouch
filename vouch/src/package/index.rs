use anyhow::{format_err, Result};
use std::collections::HashSet;

use super::common;
use crate::common::StoreTransaction;
use crate::registry;

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub package_name: Option<&'a str>,
    pub package_version: Option<&'a str>,
    pub registry_host_name: Option<&'a str>,
}

pub fn setup(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        r"
        CREATE TABLE IF NOT EXISTS package (
            id                         INTEGER NOT NULL PRIMARY KEY,
            name                       TEXT NOT NULL,
            version                    TEXT NOT NULL,
            registry_id                INTEGER NOT NULL,
            registry_human_url         TEXT NOT NULL,
            archive_url                TEXT NOT NULL,
            archive_hash               TEXT NOT NULL,

            FOREIGN KEY(registry_id) REFERENCES registry(id)
            UNIQUE(name, version, registry_id)
        )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(
    package_name: &str,
    package_version: &str,
    registry_human_url: &url::Url,
    archive_url: &url::Url,
    archive_hash: &str,
    registry_host_name: &str,
    tx: &StoreTransaction,
) -> Result<common::Package> {
    let registry = match registry::index::get(
        &registry::index::Fields {
            host_name: Some(registry_host_name),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    {
        Some(registry) => registry,
        None => registry::index::insert(&registry_host_name, &tx)?,
    };

    tx.index_tx().execute_named(
        r"
            INSERT INTO package (
                name,
                version,
                registry_id,
                registry_human_url,
                archive_url,
                archive_hash
            )
            VALUES (
                :name,
                :version,
                :registry_id,
                :registry_human_url,
                :archive_url,
                :archive_hash
            )
        ",
        rusqlite::named_params! {
            ":name": package_name,
            ":version": package_version,
            ":registry_id": registry.id,
            ":registry_human_url": registry_human_url.to_string(),
            ":archive_url": archive_url.to_string(),
            ":archive_hash": archive_hash,
        },
    )?;
    Ok(common::Package {
        id: tx.index_tx().last_insert_rowid(),
        name: package_name.to_string(),
        version: package_version.to_string(),
        registry: registry,
        registry_human_url: registry_human_url.clone(),
        archive_url: archive_url.clone(),
        archive_hash: archive_hash.to_string(),
    })
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<common::Package>> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let package_name = crate::common::index::get_like_clause_param(fields.package_name);
    let package_version = crate::common::index::get_like_clause_param(fields.package_version);
    let registry_host_name = crate::common::index::get_like_clause_param(fields.registry_host_name);

    let mut statement = tx.index_tx().prepare(
        r"
            SELECT *
            FROM package
            JOIN registry
                ON package.registry_id = registry.id
            WHERE
                package.id LIKE :package_id ESCAPE '\'
                AND name LIKE :name ESCAPE '\'
                AND version LIKE :version ESCAPE '\'
                AND registry.host_name LIKE :registry_host_name ESCAPE '\'
        ",
    )?;
    let mut rows = statement.query_named(&[
        (":package_id", &id),
        (":name", &package_name),
        (":version", &package_version),
        (":registry_host_name", &registry_host_name),
    ])?;
    let mut packages = HashSet::new();
    while let Some(row) = rows.next()? {
        let registry_id: crate::common::index::ID = row.get(3)?;
        let registry = registry::index::get(
            &registry::index::Fields {
                id: Some(registry_id),
                ..Default::default()
            },
            &tx,
        )?
        .into_iter()
        .next()
        .ok_or(format_err!("Failed to find registry for package.",))?;

        let package = common::Package {
            id: row.get(0)?,
            name: row.get(1)?,
            version: row.get(2)?,
            registry: registry,
            registry_human_url: url::Url::parse(row.get::<_, String>(4)?.as_str())?,
            archive_url: url::Url::parse(row.get::<_, String>(5)?.as_str())?,
            archive_hash: row.get(6)?,
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
        log::debug!("Inserting package: {:?}", package);
        let package = insert(
            &package.name,
            &package.version,
            &package.registry_human_url,
            &package.archive_url,
            &package.archive_hash,
            &package.registry.host_name,
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
    use anyhow::Result;

    #[test]
    fn test_merge_correct_difference_set() -> Result<()> {
        let existing_packages = maplit::hashset! {
            common::Package {
                id: 2,
                name: "py-cpuinfo".to_string(),
                version: "5.0.0".to_string(),
                registry: registry::Registry {
                    id: 2,
                    host_name: "pypi.org".to_string()
                },
                registry_human_url: url::Url::parse( "https://pypi.org/pypi/py-cpuinfo/5.0.0/")?,
                archive_url: url::Url::parse("https://files.pythonhosted.org/packages/42/60/63f28a5401da733043abe7053e7d9591491b4784c4f87c339bf51215aa0a/py-cpuinfo-5.0.0.tar.gz")?,
                archive_hash: "4a42aafca3d68e4feee71fde2779c6b30be37370aa6deb3e88356bbec266d017".to_string()
            }
        };
        let incoming_packages = maplit::hashset! {
            common::Package {
                id: 3,
                name: "py-cpuinfo".to_string(),
                version: "5.0.0".to_string(),
                registry: registry::Registry {
                    id: 1,
                    host_name: "pypi.org".to_string()
                },
                registry_human_url: url::Url::parse("https://pypi.org/pypi/py-cpuinfo/5.0.0/")?,
                archive_url: url::Url::parse("https://files.pythonhosted.org/packages/42/60/63f28a5401da733043abe7053e7d9591491b4784c4f87c339bf51215aa0a/py-cpuinfo-5.0.0.tar.gz")?,
                archive_hash: "4a42aafca3d68e4feee71fde2779c6b30be37370aa6deb3e88356bbec266d017".to_string()
            }
        };
        let result =
            crate::common::index::get_difference_sans_id(&incoming_packages, &existing_packages)?;
        assert!(result.is_empty());
        Ok(())
    }
}
