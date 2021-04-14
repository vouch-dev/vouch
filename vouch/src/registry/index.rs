use anyhow::Result;

use super::common;
use crate::common::StoreTransaction;
use std::collections::HashSet;

pub fn setup(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        "CREATE TABLE IF NOT EXISTS registry (
        id                         INTEGER NOT NULL PRIMARY KEY,
        host_name                  TEXT NOT NULL,
        registry_human_url         TEXT NOT NULL,
        archive_url                TEXT NOT NULL UNIQUE
    )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(
    host_name: &str,
    registry_human_url: &url::Url,
    archive_url: &url::Url,
    tx: &StoreTransaction,
) -> Result<common::Registry> {
    tx.index_tx().execute_named(
        "INSERT INTO registry (
                host_name,
                registry_human_url,
                archive_url
            )
            VALUES (
                :host_name,
                :registry_human_url,
                :archive_url
            )",
        rusqlite::named_params! {
            ":host_name": host_name,
            ":registry_human_url": registry_human_url.to_string(),
            ":archive_url": archive_url.to_string(),
        },
    )?;
    Ok(common::Registry {
        id: tx.index_tx().last_insert_rowid(),
        host_name: host_name.to_string(),
        registry_human_url: registry_human_url.clone(),
        archive_url: archive_url.clone(),
    })
}

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub host_name: Option<&'a str>,
    pub registry_human_url: Option<&'a str>,
    pub archive_url: Option<&'a str>,
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<common::Registry>> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let host_name = crate::common::index::get_like_clause_param(fields.host_name);
    let registry_human_url = crate::common::index::get_like_clause_param(fields.registry_human_url);
    let archive_url = crate::common::index::get_like_clause_param(fields.archive_url);

    let mut statement = tx.index_tx().prepare(
        r"
            SELECT *
            FROM registry
            WHERE
                id LIKE :id ESCAPE '\'
                AND host_name LIKE :host_name ESCAPE '\'
                AND registry_human_url LIKE :registry_human_url ESCAPE '\'
                AND archive_url LIKE :archive_url ESCAPE '\'
        ",
    )?;
    let mut rows = statement.query_named(&[
        (":id", &id),
        (":host_name", &host_name),
        (":registry_human_url", &registry_human_url),
        (":archive_url", &archive_url),
    ])?;
    let mut registries = HashSet::new();
    while let Some(row) = rows.next()? {
        registries.insert(common::Registry {
            id: row.get(0)?,
            host_name: row.get(1)?,
            registry_human_url: url::Url::parse(row.get::<_, String>(2)?.as_str())?,
            archive_url: url::Url::parse(row.get::<_, String>(3)?.as_str())?,
        });
    }
    Ok(registries)
}

/// Merge registries from incoming index into another index. Returns the newly merged registries.
pub fn merge(
    incoming_tx: &StoreTransaction,
    tx: &StoreTransaction,
) -> Result<HashSet<common::Registry>> {
    let existing_registries = get(&Fields::default(), &tx)?;
    let incoming_registries = get(&Fields::default(), &incoming_tx)?;

    let mut new_registries = HashSet::new();
    for registry in
        crate::common::index::get_difference_sans_id(&incoming_registries, &existing_registries)?
    {
        let registry = insert(
            registry.host_name.as_str(),
            &registry.registry_human_url,
            &registry.archive_url,
            &tx,
        )?;
        new_registries.insert(registry);
    }
    Ok(new_registries)
}

pub fn ensure(
    host_name: &str,
    registry_human_url: &url::Url,
    archive_url: &url::Url,
    tx: &StoreTransaction,
) -> Result<common::Registry> {
    let registry = get(
        &Fields {
            host_name: Some(host_name),
            registry_human_url: Some(registry_human_url.as_str()),
            archive_url: Some(archive_url.as_str()),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next();

    Ok(match registry {
        Some(registry) => registry,
        None => insert(&host_name, &registry_human_url, &archive_url, &tx)?,
    })
}
