use super::super::common;
use crate::common::StoreTransaction;
use crate::registry;
use anyhow::{format_err, Result};
use std::collections::HashSet;
use std::hash::Hash;

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Package {
    #[serde(skip)]
    pub id: common::index::ID,

    pub name: String,
    pub version: String,
    pub registry: registry::index::Registry,

    pub registry_human_url: url::Url,

    pub source_code_url: url::Url,
    pub source_code_hash: String,
}

impl common::index::Identify for Package {
    fn id(&self) -> common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut common::index::ID {
        &mut self.id
    }
}

impl crate::common::HashSansId for Package {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.version.hash(state);
        self.registry.hash(state);
        self.registry_human_url.hash(state);
        self.source_code_url.hash(state);
        self.source_code_hash.hash(state);
    }
}

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<common::index::ID>,
    pub package_name: Option<&'a str>,
    pub package_version: Option<&'a str>,
    pub registry_host_name: Option<&'a str>,
}

pub fn setup_database(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        r"
        CREATE TABLE IF NOT EXISTS package (
            id                         INTEGER NOT NULL PRIMARY KEY,
            name                       TEXT NOT NULL,
            version                    TEXT NOT NULL,
            registry_id                INTEGER NOT NULL,
            registry_human_url         TEXT NOT NULL,
            source_code_url            TEXT NOT NULL,
            source_code_hash           TEXT NOT NULL,

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
    source_code_url: &url::Url,
    source_code_hash: &str,
    registry_host_name: &str,
    tx: &StoreTransaction,
) -> Result<Package> {
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
                source_code_url,
                source_code_hash
            )
            VALUES (
                :name,
                :version,
                :registry_id,
                :registry_human_url,
                :source_code_url,
                :source_code_hash
            )
        ",
        rusqlite::named_params! {
            ":name": package_name,
            ":version": package_version,
            ":registry_id": registry.id,
            ":registry_human_url": registry_human_url.to_string(),
            ":source_code_url": source_code_url.to_string(),
            ":source_code_hash": source_code_hash,
        },
    )?;
    Ok(Package {
        id: tx.index_tx().last_insert_rowid(),
        name: package_name.to_string(),
        version: package_version.to_string(),
        registry: registry,
        registry_human_url: registry_human_url.clone(),
        source_code_url: source_code_url.clone(),
        source_code_hash: source_code_hash.to_string(),
    })
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<Package>> {
    let id = common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let package_name = common::index::get_like_clause_param(fields.package_name);
    let package_version = common::index::get_like_clause_param(fields.package_version);
    let registry_host_name = common::index::get_like_clause_param(fields.registry_host_name);

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
        let registry_id: common::index::ID = row.get(3)?;
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

        let package = Package {
            id: row.get(0)?,
            name: row.get(1)?,
            version: row.get(2)?,
            registry: registry,
            registry_human_url: url::Url::parse(row.get::<_, String>(4)?.as_str())?,
            source_code_url: url::Url::parse(row.get::<_, String>(5)?.as_str())?,
            source_code_hash: row.get(6)?,
        };
        packages.insert(package);
    }
    Ok(packages)
}

/// Merge packages from incoming index into another index. Returns the newly merged packages.
pub fn merge(incoming_tx: &StoreTransaction, tx: &StoreTransaction) -> Result<HashSet<Package>> {
    let existing_packages = get(&Fields::default(), &tx)?;
    let incoming_packages = get(&Fields::default(), &incoming_tx)?;

    let mut new_packages = HashSet::new();
    for package in common::index::get_difference_sans_id(&incoming_packages, &existing_packages)? {
        let package = insert(
            &package.name,
            &package.version,
            &package.registry_human_url,
            &package.source_code_url,
            &package.source_code_hash,
            &package.registry.host_name,
            &tx,
        )?;
        new_packages.insert(package);
    }
    Ok(new_packages)
}
