use anyhow::Result;

use crate::common::{self, StoreTransaction};
use std::collections::HashSet;
use std::hash::Hash;

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Registry {
    #[serde(skip)]
    pub id: common::index::ID,

    pub host_name: String,
}

impl crate::common::HashSansId for Registry {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.host_name.hash(state);
    }
}

impl common::index::Identify for Registry {
    fn id(&self) -> common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut common::index::ID {
        &mut self.id
    }
}

pub fn setup_database(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        "CREATE TABLE IF NOT EXISTS registry (
        id              INTEGER NOT NULL PRIMARY KEY,
        host_name       TEXT NOT NULL UNIQUE
    )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(host_name: &str, tx: &StoreTransaction) -> Result<Registry> {
    tx.index_tx().execute_named(
        "INSERT INTO registry (host_name)
            VALUES (
                :host_name
            )",
        rusqlite::named_params! {
            ":host_name": host_name
        },
    )?;
    Ok(Registry {
        id: tx.index_tx().last_insert_rowid(),
        host_name: host_name.to_string(),
    })
}

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub host_name: Option<&'a str>,
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<Registry>> {
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
        registries.insert(Registry {
            id: row.get(0)?,
            host_name: row.get(1)?,
        });
    }
    Ok(registries)
}

/// Merge registires from incoming index into another index. Returns the newly merged registires.
pub fn merge(incoming_tx: &StoreTransaction, tx: &StoreTransaction) -> Result<HashSet<Registry>> {
    let existing_registires = get(&Fields::default(), &tx)?;
    let incoming_registires = get(&Fields::default(), &incoming_tx)?;

    let mut new_registires = HashSet::new();
    for registry in
        common::index::get_difference_sans_id(&incoming_registires, &existing_registires)?
    {
        let registry = insert(registry.host_name.as_str(), &tx)?;
        new_registires.insert(registry);
    }
    Ok(new_registires)
}
