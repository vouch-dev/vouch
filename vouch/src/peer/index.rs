use anyhow::{format_err, Result};
use bincode;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::TryFrom;

use super::common;
use crate::common::StoreTransaction;

#[derive(Debug, Default)]
pub struct Fields<'a> {
    pub id: Option<crate::common::index::ID>,
    pub alias: Option<&'a str>,
    pub git_url: Option<&'a crate::common::GitUrl>,
    pub parent_id: Option<crate::common::index::ID>,
    pub child_peer_ids: Option<common::SubPeerIds>,
}

/// Returns the root peer.
pub fn get_root(tx: &StoreTransaction) -> Result<Option<common::Peer>> {
    Ok(get(
        &Fields {
            alias: Some(common::ROOT_ALIAS),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    .map(|x| x.clone()))
}

pub fn setup(tx: &StoreTransaction) -> Result<()> {
    tx.index_tx().execute(
        "
    CREATE TABLE IF NOT EXISTS peer (
        id              INTEGER NOT NULL PRIMARY KEY,
        alias           TEXT NOT NULL UNIQUE,
        git_url         TEXT NOT NULL UNIQUE,
        parent_id       INTEGER,
        child_peer_ids  BLOB,

        FOREIGN KEY(parent_id) REFERENCES peer(id)
    )",
        rusqlite::NO_PARAMS,
    )?;

    // Insert root peer if absent.
    let found_root_peer = !get(
        &Fields {
            alias: Some(common::ROOT_ALIAS),
            ..Default::default()
        },
        &tx,
    )?
    .is_empty();
    if !found_root_peer {
        let git_url = crate::common::GitUrl::try_from(common::ROOT_DEFAULT_GIT_URL)?;
        log::debug!(
            "Failed to find root peer. Inserting: {alias} ({git_url})",
            alias = common::ROOT_ALIAS,
            git_url = git_url
        );
        let parent_peer: Option<&mut common::Peer> = None;
        insert(common::ROOT_ALIAS, &git_url, parent_peer, tx)?;
    }
    Ok(())
}

pub fn insert(
    alias: &str,
    git_url: &crate::common::GitUrl,
    parent_peer: Option<&mut common::Peer>,
    tx: &StoreTransaction,
) -> Result<common::Peer> {
    let parent_id = match &parent_peer {
        Some(parent_peer) => Some(parent_peer.id.clone()),
        None => None,
    };
    tx.index_tx().execute(
        "
        INSERT INTO peer (alias, git_url, parent_id, child_peer_ids)
            VALUES (?1, ?2, ?3, ?4)
        ",
        rusqlite::params![
            alias,
            git_url.to_string(),
            parent_id,
            None as Option<Vec<u8>>
        ],
    )?;
    let new_peer = common::Peer {
        id: tx.index_tx().last_insert_rowid(),
        alias: alias.to_string(),
        git_url: git_url.clone(),
        parent_id: parent_id,
        child_peer_ids: None,
    };

    if let Some(parent_peer) = parent_peer {
        add_child_peer_id(parent_peer, &new_peer, &tx)?;
    }

    Ok(new_peer)
}

/// Given a peer, extend its child peer set.
fn add_child_peer_id(
    peer: &mut common::Peer,
    child_peer: &common::Peer,
    tx: &StoreTransaction,
) -> Result<()> {
    if peer.child_peer_ids.is_none() {
        peer.child_peer_ids = Some(common::SubPeerIds(BTreeSet::new()));
    }

    if let Some(child_peer_ids) = &mut peer.child_peer_ids {
        child_peer_ids.0.insert(child_peer.id);
        tx.index_tx().execute(
            r"
            UPDATE peer
            SET child_peer_ids = ?2
            WHERE id = ?1
        ",
            rusqlite::params![peer.id, bincode::serialize(&child_peer_ids)?,],
        )?;
    }
    Ok(())
}

/// Given a peer, remove a peer from its child peer set.
fn remove_child_peer_id(
    peer: &mut common::Peer,
    child_peer: &common::Peer,
    tx: &StoreTransaction,
) -> Result<()> {
    if peer.child_peer_ids.is_none() {
        return Ok(());
    }

    if let Some(child_peer_ids) = &mut peer.child_peer_ids {
        let child_peer_removed = child_peer_ids.0.remove(&child_peer.id);
        if child_peer_removed {
            let child_peer_ids = if child_peer_ids.0.is_empty() {
                None as Option<Vec<u8>>
            } else {
                Some(bincode::serialize(&child_peer_ids)?)
            };

            tx.index_tx().execute(
                r"
            UPDATE peer
            SET child_peer_ids = ?2
            WHERE id = ?1
        ",
                rusqlite::params![peer.id, child_peer_ids,],
            )?;
        }
    }
    Ok(())
}

/// Get matching peers.
pub fn get(fields: &Fields, tx: &StoreTransaction) -> Result<HashSet<common::Peer>> {
    let id =
        crate::common::index::get_like_clause_param(fields.id.map(|id| id.to_string()).as_deref());
    let alias = crate::common::index::get_like_clause_param(fields.alias);
    let git_url =
        crate::common::index::get_like_clause_param(fields.git_url.map(|url| url.as_str()));
    let parent_id = crate::common::index::get_like_clause_param(
        fields.parent_id.map(|id| id.to_string()).as_deref(),
    );

    let sql_query = r"
        SELECT *
        FROM peer
        WHERE
            id LIKE :id ESCAPE '\'
            AND alias LIKE :alias ESCAPE '\'
            AND git_url LIKE :git_url ESCAPE '\'
            AND ifnull(parent_id, '') LIKE :parent_id ESCAPE '\'
    ";
    let mut statement = tx.index_tx().prepare(sql_query)?;
    let mut rows = statement.query_named(&[
        (":id", &id),
        (":alias", &alias),
        (":git_url", &git_url),
        (":parent_id", &parent_id),
    ])?;
    let mut peers = HashSet::new();
    while let Some(row) = rows.next()? {
        let git_url = crate::common::GitUrl::try_from(&row.get::<_, String>(2)?)?;
        let child_peer_ids: Option<Result<common::SubPeerIds>> = row
            .get::<_, Option<Vec<u8>>>(4)?
            .map(|x| Ok(bincode::deserialize(&x)?));
        let child_peer_ids = match child_peer_ids {
            Some(v) => Some(v.map_err(|_| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Blob,
                    Box::from(format!(
                        "Failed to parse field `child_peer_ids` for peer: {git_url}",
                        git_url = git_url
                    )),
                )
            })?),
            None => None,
        };
        peers.insert(common::Peer {
            id: row.get(0)?,
            alias: row.get(1)?,
            git_url,
            parent_id: row.get(3)?,
            child_peer_ids,
        });
    }
    Ok(peers)
}

/// Remove peer.
pub fn remove(fields: &Fields, tx: &StoreTransaction) -> Result<()> {
    let peer = match get(&fields, &tx)?.into_iter().next() {
        Some(peer) => peer,
        None => return Ok(()),
    };

    if let Some(child_peer_ids) = &peer.child_peer_ids {
        assert!(
            child_peer_ids.0.is_empty(),
            "Error removing peer. Peer has associated child peers which need to be removed first."
        );
    }

    // Remove peer from its parent's child peer set.
    let parent_peer_id = peer.parent_id.ok_or(format_err!(
        "Peer does not have a parent peer. \
    Peer must therefore be the root peer. Cannot remove root peer."
    ))?;
    let mut parent_peer = get(
        &Fields {
            id: Some(parent_peer_id),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    .ok_or(format_err!("Parent peer not found in index."))?
    .clone();
    remove_child_peer_id(&mut parent_peer, &peer, &tx)?;

    let peer_id = crate::common::index::get_like_clause_param(Some(&peer.id.to_string()));
    tx.index_tx().execute_named(
        r"
        DELETE
        FROM peer
        WHERE
            id LIKE :peer_id ESCAPE '\'
    ",
        &[(":peer_id", &peer_id)],
    )?;
    Ok(())
}

pub fn get_root_to_peer_subtree(
    peer: &common::Peer,
    tx: &StoreTransaction,
) -> Result<Vec<common::Peer>> {
    let mut subtree = std::collections::VecDeque::new();
    let mut current_peer = peer.clone();
    loop {
        subtree.push_front(current_peer.clone());
        match current_peer.parent_id {
            Some(parent_id) => {
                current_peer = get(
                    &Fields {
                        id: Some(parent_id),
                        ..Default::default()
                    },
                    &tx,
                )?
                .into_iter()
                .next()
                .ok_or(format_err!(
                    "Failed to find parent for peer: {:?}",
                    current_peer
                ))?
                .clone();
            }
            None => {
                break;
            }
        }
    }
    Ok(subtree.into())
}

/// Merge peers from incoming index into another index. Returns the newly merged peers.
pub fn merge(
    incoming_root_git_url: &crate::common::GitUrl,
    incoming_tx: &StoreTransaction,
    tx: &StoreTransaction,
) -> Result<HashSet<common::Peer>> {
    let existing_peers = get(&Fields::default(), &tx)?;
    let mut existing_peers: HashMap<crate::common::GitUrl, common::Peer> = existing_peers
        .into_iter()
        .map(|peer| (peer.git_url.clone(), peer))
        .collect();

    let mut inserted_peers = HashMap::<crate::common::GitUrl, common::Peer>::new();

    let mut insert_peer = |peer: &common::Peer, parent_peer: Option<&common::Peer>| -> Result<()> {
        if existing_peers.contains_key(&peer.git_url) || inserted_peers.contains_key(&peer.git_url)
        {
            return Ok(());
        }

        // Get parent peer from destination index.
        let parent_peer = match parent_peer {
            Some(parent_peer) => match inserted_peers.get_mut(&parent_peer.git_url) {
                Some(inserted_parent_peer) => Some(inserted_parent_peer),
                None => existing_peers.get_mut(&parent_peer.git_url),
            },
            None => None,
        };
        let inserted_peer = insert(
            get_new_alias(&peer.git_url, &tx)?.as_str(),
            &peer.git_url,
            parent_peer,
            &tx,
        )?;
        inserted_peers.insert(peer.git_url.clone(), inserted_peer);
        Ok(())
    };

    let root_peer = get_root(&tx)?.ok_or(format_err!(
        "Root peer must exist before merging in other peers."
    ))?;

    for subtree in get_peer_subtrees(None, &incoming_tx)? {
        for peer_pair in subtree.windows(2) {
            let parent_peer = &peer_pair[0];
            let peer = &peer_pair[1];

            // Modify git url of incoming root peer.
            let parent_peer = if parent_peer.is_root() {
                let mut parent_peer = parent_peer.clone();
                parent_peer.git_url = incoming_root_git_url.clone();
                parent_peer
            } else {
                parent_peer.clone()
            };

            if parent_peer.is_root() {
                insert_peer(&parent_peer, Some(&root_peer))?;
            }

            insert_peer(peer, Some(&parent_peer))?;
        }
    }

    let inserted_peers: HashSet<common::Peer> =
        inserted_peers.values().map(|p| p.clone()).collect();
    Ok(inserted_peers)
}

pub fn get_new_alias(git_url: &crate::common::GitUrl, tx: &StoreTransaction) -> Result<String> {
    let mut alias = git_url.as_str();
    if git_url.url().host_str() == Some("github.com")
        || git_url.url().host_str() == Some("gitlab.com")
    {
        let segments = git_url.url().path_segments().map(|c| c.collect::<Vec<_>>());
        if let Some(segments) = segments {
            if let Some(first_segment) = segments.first() {
                alias = first_segment;
            }
        }
    };

    // Root alias is reserved for the root peer only.
    if alias == common::ROOT_ALIAS {
        return Ok(git_url.to_string());
    }

    // Ensure alias is not used. Use full git url as fallback.
    if !get(
        &Fields {
            alias: Some(&alias),
            ..Default::default()
        },
        &tx,
    )?
    .is_empty()
    {
        return Ok(git_url.to_string());
    }
    Ok(alias.to_string())
}

/// Returns child peer subtree in breadth first layers.
///
/// Starting peer is in the first layer. Leaf peers are in the final layer.
pub fn get_breadth_first_child_peers(
    starting_peer: &common::Peer,
    tx: &StoreTransaction,
) -> Result<Vec<HashSet<common::Peer>>> {
    let mut breadth_layers = Vec::new();
    let mut unprocessed_peers = maplit::hashset! {starting_peer.clone()};
    loop {
        if unprocessed_peers.is_empty() {
            break;
        }
        breadth_layers.push(unprocessed_peers.clone());

        let mut all_child_peers = HashSet::new();
        for peer in unprocessed_peers.drain() {
            let children = get(
                &Fields {
                    parent_id: Some(peer.id),
                    ..Default::default()
                },
                &tx,
            )?;
            all_child_peers.extend(children);
        }
        unprocessed_peers = all_child_peers;
    }
    Ok(breadth_layers)
}

fn get_peer_subtrees(
    starting_subtree: Option<&Vec<common::Peer>>,
    tx: &StoreTransaction,
) -> Result<Vec<Vec<common::Peer>>> {
    let mut complete_subtrees = Vec::<Vec<common::Peer>>::new();

    let starting_subtree = match starting_subtree {
        Some(starting_subtree) => starting_subtree.clone(),
        None => {
            let root_peer = get_root(&tx)?.ok_or(format_err!("Cannot find root peer."))?;
            vec![root_peer]
        }
    };

    let mut incomplete_subtrees = std::collections::VecDeque::<Vec<common::Peer>>::new();
    incomplete_subtrees.push_back(starting_subtree);

    loop {
        let subtree = match incomplete_subtrees.pop_front() {
            Some(subtree) => subtree,
            None => {
                break;
            }
        };

        let leaf_peer = subtree
            .last()
            .ok_or(format_err!("Found an empty subtree."))?;
        let children = get(
            &Fields {
                parent_id: Some(leaf_peer.id),
                ..Default::default()
            },
            &tx,
        )?;
        if children.is_empty() {
            complete_subtrees.push(subtree);
            continue;
        }

        for child in children {
            let mut new_subtree = subtree.clone();
            new_subtree.push(child);
            incomplete_subtrees.push_back(new_subtree);
        }
    }

    Ok(complete_subtrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_resulting_peer_structure_correct() -> Result<()> {
        // Setup incoming database.
        let mut incoming_db = rusqlite::Connection::open_in_memory()?;
        let incoming_tx = StoreTransaction::new(incoming_db.transaction()?)?;

        setup(&incoming_tx)?;
        // root_incoming -> peer_1 -> peer_2
        let mut root_peer = get_root(&incoming_tx)?.unwrap();
        let mut peer_1 = insert(
            "peer_1",
            &crate::common::GitUrl::try_from("https://localhost/peer_1")?,
            Some(&mut root_peer),
            &incoming_tx,
        )?;
        insert(
            "peer_2",
            &crate::common::GitUrl::try_from("https://localhost/peer_2")?,
            Some(&mut peer_1),
            &incoming_tx,
        )?;

        // Setup destination database.
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        setup(&tx)?;
        // root -> peer_3 -> peer_2
        let mut root_peer = get_root(&tx)?.unwrap();
        let mut peer_3 = insert(
            "peer_3",
            &crate::common::GitUrl::try_from("https://localhost/peer_3")?,
            Some(&mut root_peer),
            &tx,
        )?;
        insert(
            "peer_2",
            &crate::common::GitUrl::try_from("https://localhost/peer_2")?,
            Some(&mut peer_3),
            &tx,
        )?;

        // Merge incoming and destination databases.
        // root -> root_incoming -> peer_1
        // root -> peer_3 -> peer_2
        let incoming_root_git_url =
            crate::common::GitUrl::try_from("https://localhost/root_incoming")?;
        merge(&incoming_root_git_url, &incoming_tx, &tx)?;

        let result: HashSet<common::Peer> = get(&Fields::default(), &tx)?.into_iter().collect();
        let expected = maplit::hashset! {
            common::Peer {
                id: 1,
                alias: "root".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost")?,
                parent_id: None,
                child_peer_ids: Some(common::SubPeerIds(maplit::btreeset! {2, 4})),
            },
            common::Peer {
                id: 2,
                alias: "peer_3".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/peer_3")?,
                parent_id: Some(1),
                child_peer_ids: Some(common::SubPeerIds(maplit::btreeset! {3})),
            },
            common::Peer {
                id: 3,
                alias: "peer_2".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/peer_2")?,
                parent_id: Some(2),
                child_peer_ids: None,
            },
            common::Peer {
                id: 4,
                alias: "https://localhost/root_incoming".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/root_incoming")?,
                parent_id: Some(1),
                child_peer_ids: Some(common::SubPeerIds(maplit::btreeset! {5})),
            },
            common::Peer {
                id: 5,
                alias: "https://localhost/peer_1".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/peer_1")?,
                parent_id: Some(4),
                child_peer_ids: None,
            },
        };
        let unexpected_peers = crate::common::index::get_difference_sans_id(&result, &expected)?;
        assert!(unexpected_peers.is_empty(), "Found unexpected peers.");
        Ok(())
    }

    #[test]
    fn test_merge_return_newly_added_peers() -> Result<()> {
        // Setup incoming database.
        let mut incoming_db = rusqlite::Connection::open_in_memory()?;
        let incoming_tx = StoreTransaction::new(incoming_db.transaction()?)?;

        let incoming_root_git_url =
            crate::common::GitUrl::try_from("https://localhost/root_incoming")?;
        setup(&incoming_tx)?;
        // root_incoming -> peer_1 -> peer_2
        let mut root_peer = get_root(&incoming_tx)?.unwrap();
        let mut peer_1 = insert(
            "peer_1",
            &crate::common::GitUrl::try_from("https://localhost/peer_1")?,
            Some(&mut root_peer),
            &incoming_tx,
        )?;
        insert(
            "peer_2",
            &crate::common::GitUrl::try_from("https://localhost/peer_2")?,
            Some(&mut peer_1),
            &incoming_tx,
        )?;

        // Setup destination database.
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        setup(&tx)?;
        // root -> peer_3 -> peer_2
        let mut root_peer = get_root(&tx)?.unwrap();
        let mut peer_3 = insert(
            "peer_3",
            &crate::common::GitUrl::try_from("https://localhost/peer_3")?,
            Some(&mut root_peer),
            &tx,
        )?;
        insert(
            "peer_2",
            &crate::common::GitUrl::try_from("https://localhost/peer_2")?,
            Some(&mut peer_3),
            &tx,
        )?;

        // Merge incoming and destination databases.
        // root -> root_incoming -> peer_1
        // root -> peer_3 -> peer_2
        let result = merge(&incoming_root_git_url, &incoming_tx, &tx)?;

        let expected = maplit::hashset! {
            common::Peer {
                id: 4,
                alias: "https://localhost/root_incoming".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/root_incoming")?,
                parent_id: Some(1),
                child_peer_ids: Some(common::SubPeerIds(maplit::btreeset! {5})),
            },
            common::Peer {
                id: 5,
                alias: "https://localhost/peer_1".to_string(),
                git_url: crate::common::GitUrl::try_from("https://localhost/peer_1")?,
                parent_id: Some(4),
                child_peer_ids: None,
            },
        };
        let unexpected_peers = crate::common::index::get_difference_sans_id(&result, &expected)?;
        assert!(unexpected_peers.is_empty());
        Ok(())
    }

    #[test]
    fn test_get_peer_subtrees() -> Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        setup(&tx)?;
        let mut root_peer = get_root(&tx)?.unwrap();

        // root -> peer_1 -> peer_2
        let mut peer_1 = insert(
            "peer_1",
            &crate::common::GitUrl::try_from("https://localhost/peer_1")?,
            Some(&mut root_peer),
            &tx,
        )?;
        let peer_2 = insert(
            "peer_2",
            &crate::common::GitUrl::try_from("https://localhost/peer_2")?,
            Some(&mut peer_1),
            &tx,
        )?;

        // root -> peer_3
        let peer_3 = insert(
            "peer_3",
            &crate::common::GitUrl::try_from("https://localhost/peer_3")?,
            Some(&mut root_peer),
            &tx,
        )?;

        let result = get_peer_subtrees(None, &tx)?;
        let expected = vec![
            vec![root_peer.clone(), peer_3],
            vec![root_peer.clone(), peer_1, peer_2],
        ];
        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn test_insert_peers_correct_child_peer_ids() -> Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        setup(&tx)?;
        let mut root_peer = get_root(&tx)?.unwrap();
        insert(
            "new_peer",
            &crate::common::GitUrl::try_from("https://localhost/new_peer")?,
            Some(&mut root_peer),
            &tx,
        )?;

        let fields = Fields::default();
        let result = get(&fields, &tx)?;
        let expected = maplit::hashset! {
            common::Peer {
                id: 1,
                alias: common::ROOT_ALIAS.to_owned(),
                git_url: crate::common::GitUrl::try_from(common::ROOT_DEFAULT_GIT_URL)?,
                parent_id: None,
                child_peer_ids: Some(common::SubPeerIds(maplit::btreeset! { 2 as i64 })),
            },
            common::Peer {
                id: 2,
                alias: "new_peer".to_owned(),
                git_url: crate::common::GitUrl::try_from("https://localhost/new_peer")?,
                parent_id: Some(1),
                child_peer_ids: None,
            },
        };
        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn test_delete_peer() -> Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let tx = StoreTransaction::new(db.transaction()?)?;
        setup(&tx)?;
        let mut root_peer = get_root(&tx)?.unwrap();

        insert(
            "new_peer",
            &crate::common::GitUrl::try_from("https://localhost/new_peer")?,
            Some(&mut root_peer),
            &tx,
        )?;
        remove(
            &Fields {
                alias: Some("new_peer"),
                ..Default::default()
            },
            &tx,
        )?;

        let fields = Fields::default();
        let result = get(&fields, &tx)?;
        let expected = maplit::hashset! {common::Peer {
            id: 1,
            alias: common::ROOT_ALIAS.to_owned(),
            git_url: crate::common::GitUrl::try_from("https://localhost")?,
            parent_id: None,
            child_peer_ids: None,
        }};
        assert_eq!(result, expected);
        Ok(())
    }
}
