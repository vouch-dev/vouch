use anyhow::Result;

use super::common;
use crate::common::StoreTransaction;
use std::collections::HashSet;

pub fn setup(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        "CREATE TABLE IF NOT EXISTS registry (
        id              INTEGER NOT NULL PRIMARY KEY,
        host_name       TEXT NOT NULL UNIQUE
    )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(host_name: &str, tx: &StoreTransaction) -> Result<common::Registry> {
    tx.index_tx().execute_named(
        "INSERT INTO registry (host_name)
            VALUES (
                :host_name
            )",
        rusqlite::named_params! {
            ":host_name": host_name
        },
    )?;
    Ok(common::Registry {
        id: tx.index_tx().last_insert_rowid(),
        host_name: host_name.to_string(),
    })
}

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub host_name: Option<&'a str>,
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<common::Registry>> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let host_name = crate::common::index::get_like_clause_param(fields.host_name);

    let mut statement = tx.index_tx().prepare(
        r"
            SELECT *
            FROM registry
            WHERE
                id LIKE :id ESCAPE '\'
                AND host_name LIKE :host_name ESCAPE '\'
        ",
    )?;
    let mut rows = statement.query_named(&[(":id", &id), (":host_name", &host_name)])?;
    let mut registries = HashSet::new();
    while let Some(row) = rows.next()? {
        registries.insert(common::Registry {
            id: row.get(0)?,
            host_name: row.get(1)?,
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
        let registry = insert(registry.host_name.as_str(), &tx)?;
        new_registries.insert(registry);
    }
    Ok(new_registries)
}
