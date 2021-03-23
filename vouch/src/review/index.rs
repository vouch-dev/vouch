use anyhow::{format_err, Result};

use std::collections::HashSet;
use std::convert::TryFrom;

use super::common;
use crate::common::StoreTransaction;
use crate::package;
use crate::peer;

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub peer: Option<&'a peer::Peer>,
    pub package_id: Option<crate::common::index::ID>,

    pub package_security: Option<crate::common::index::ID>,
    pub review_confidence: Option<crate::common::index::ID>,

    pub package_name: Option<&'a str>,
    pub package_version: Option<&'a str>,
    pub registry_host_name: Option<&'a str>,
}

pub fn setup_database(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        r"
        CREATE TABLE IF NOT EXISTS review (
            id                    INTEGER NOT NULL PRIMARY KEY,
            peer_id               INTEGER NOT NULL,
            package_id            INTEGER NOT NULL,
            package_security      INTEGER NOT NULL,
            review_confidence     INTEGER NOT NULL,

            UNIQUE(peer_id, package_id)
            FOREIGN KEY(peer_id) REFERENCES peer(id)
            FOREIGN KEY(package_id) REFERENCES package(id)
        )",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

pub fn insert(
    package_security: &common::PackageSecurity,
    review_confidence: &common::ReviewConfidence,
    peer: &crate::peer::Peer,
    package: &crate::package::Package,
    tx: &StoreTransaction,
) -> Result<common::Review> {
    tx.index_tx().execute_named(
        r"
            INSERT INTO review (
                peer_id,
                package_id,
                package_security,
                review_confidence
            )
            VALUES (:peer_id, :package_id, :package_security, :review_confidence)
        ",
        &[
            (":peer_id", &peer.id),
            (":package_id", &package.id),
            (":package_security", &package_security.to_rating().to_u8()),
            (":review_confidence", &review_confidence.to_rating().to_u8()),
        ],
    )?;
    Ok(common::Review {
        id: tx.index_tx().last_insert_rowid(),
        package_security: package_security.clone(),
        review_confidence: review_confidence.clone(),
        peer: peer.clone(),
        package: package.clone(),
    })
}

pub fn update(review: &common::Review, tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute_named(
        r"
            UPDATE review
            SET
                peer_id = :peer_id,
                package_id = :package_id,
                package_security = :package_security,
                review_confidence = :review_confidence
            WHERE
                id = :id
        ",
        &[
            (":id", &review.id),
            (":peer_id", &review.peer.id),
            (":package_id", &review.package.id),
            (
                ":package_security",
                &review.package_security.to_rating().to_u8(),
            ),
            (
                ":review_confidence",
                &review.review_confidence.to_rating().to_u8(),
            ),
        ],
    )?;
    Ok(())
}

pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<Vec<common::Review>> {
    let review_id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());

    let package_name = crate::common::index::get_like_clause_param(fields.package_name);
    let package_version = crate::common::index::get_like_clause_param(fields.package_version);
    let registry_host_name = crate::common::index::get_like_clause_param(fields.registry_host_name);

    let peer_id = crate::common::index::get_like_clause_param(
        fields.peer.map(|peer| peer.id.to_string()).as_deref(),
    );

    let mut statement = tx.index_tx().prepare(
        r"
        SELECT
            review.id,
            review.package_security,
            review.review_confidence,
            peer.id,
            package.id
        FROM review
        JOIN peer
            ON review.peer_id = peer.id
        JOIN package
            ON review.package_id = package.id
        JOIN registry
            ON package.registry_id = registry.id
        WHERE
            review.id LIKE :review_id ESCAPE '\'
            AND package.name LIKE :name ESCAPE '\'
            AND package.version LIKE :version ESCAPE '\'
            AND peer.id LIKE :peer_id ESCAPE '\'
            AND registry.host_name LIKE :registry_host_name ESCAPE '\'
        ",
    )?;
    let mut rows = statement.query_named(&[
        (":review_id", &review_id),
        (":name", &package_name),
        (":version", &package_version),
        (":peer_id", &peer_id),
        (":registry_host_name", &registry_host_name),
    ])?;

    let mut reviews = Vec::new();
    while let Some(row) = rows.next()? {
        let peer = peer::index::get(
            &peer::index::Fields {
                id: row.get(3)?,
                ..Default::default()
            },
            &tx,
        )?
        .into_iter()
        .next()
        .ok_or(format_err!("Failed to find review peer in index."))?;

        let package = package::index::get(
            &package::index::Fields {
                id: row.get(4)?,
                ..Default::default()
            },
            &tx,
        )?
        .into_iter()
        .next()
        .ok_or(format_err!("Failed to find review package in index."))?;

        let review = common::Review {
            id: row.get(0)?,
            package_security: common::rating::Rating::try_from(&row.get::<_, u8>(1)?)?.into(),
            review_confidence: common::rating::Rating::try_from(&row.get::<_, u8>(2)?)?.into(),
            peer,
            package,
        };
        reviews.push(review);
    }
    Ok(reviews)
}

pub fn remove(fields: &Fields, tx: &StoreTransaction) -> Result<()> {
    let package_name = crate::common::index::get_like_clause_param(fields.package_name);
    let package_version = crate::common::index::get_like_clause_param(fields.package_version);
    let registry_host_name = crate::common::index::get_like_clause_param(fields.registry_host_name);

    let peer_id = crate::common::index::get_like_clause_param(
        fields.peer.map(|peer| peer.id.to_string()).as_deref(),
    );

    let mut statement = tx.index_tx().prepare(
        r"
        DELETE FROM review
        JOIN peer
            ON review.peer_id = peer.id
        JOIN package
            ON review.package_id = package.id
        JOIN registry
            ON package.registry_id = registry.id
        WHERE
            package.name LIKE :name ESCAPE '\'
            AND package.version LIKE :version ESCAPE '\'
            AND peer.id LIKE :peer_id ESCAPE '\'
            AND registry.host_name LIKE :registry_host_name ESCAPE '\'
        ",
    )?;
    statement.query_named(&[
        (":name", &package_name),
        (":version", &package_version),
        (":peer_id", &peer_id),
        (":registry_host_name", &registry_host_name),
    ])?;
    Ok(())
}

/// Merge reviews from incoming index into another index. Returns the newly merged reviews.
pub fn merge(
    incoming_tx: &StoreTransaction,
    tx: &StoreTransaction,
) -> Result<HashSet<common::Review>> {
    let incoming_reviews = get(&Fields::default(), &incoming_tx)?;

    let mut new_reviews = HashSet::new();
    for review in incoming_reviews {
        let peer = peer::index::get(
            &peer::index::Fields {
                git_url: Some(&review.peer.git_url),
                ..Default::default()
            },
            &tx,
        )?
        .into_iter()
        .next()
        .ok_or(format_err!(
            "Failed to find matching peer for review: {:?}",
            review
        ))?;

        let package = package::index::get(
            &package::index::Fields {
                package_name: Some(&review.package.name),
                package_version: Some(&review.package.version),
                registry_host_name: Some(&review.package.registry.host_name),
                ..Default::default()
            },
            &tx,
        )?
        .into_iter()
        .next()
        .ok_or(format_err!(
            "Failed to find matching package for review: {:?}",
            review
        ))?;

        let review = insert(
            &review.package_security,
            &review.review_confidence,
            &peer,
            &package,
            &tx,
        )?;
        new_reviews.insert(review);
    }
    Ok(new_reviews)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package;
    use crate::peer;

    fn get_package(unique_tag: &str, tx: &StoreTransaction) -> Result<package::Package> {
        Ok(package::index::insert(
            &format!("test_package_name_{unique_tag}", unique_tag = unique_tag),
            "test_package_version",
            &url::Url::parse("http://localhost/test_registry_human_url")?,
            &url::Url::parse("http://localhost/test_source_code_url")?,
            "test_source_code_hash",
            "test_registry_host_name",
            &tx,
        )?)
    }

    #[test]
    fn test_insert_get_new_reviews() -> Result<()> {
        let mut store = crate::store::Store::from_tmp()?;
        let tx = store.get_transaction()?;

        let package_1 = get_package("package_1", &tx)?;
        let package_2 = get_package("package_2", &tx)?;

        let root_peer = peer::index::get_root(&tx)?.unwrap();

        insert(
            &common::PackageSecurity::Safe,
            &common::ReviewConfidence::High,
            &root_peer,
            &package_1,
            &tx,
        )?;
        insert(
            &common::PackageSecurity::Safe,
            &common::ReviewConfidence::High,
            &root_peer,
            &package_2,
            &tx,
        )?;

        // let expected = vec![review_1, review_2];
        let result = get(&Fields::default(), &tx)?;
        println!("result: {:?}", result);
        // assert_eq!(result, expected);
        Ok(())
    }
}
