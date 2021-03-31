use anyhow::{format_err, Result};
use structopt::{self, StructOpt};

use crate::common;
use crate::peer;
use crate::review;
use crate::store;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {}

pub fn run_command(_args: &Arguments) -> Result<()> {
    let mut store = store::Store::from_root()?;
    let mut tx = store.get_transaction()?;

    let root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;
    let root_children = peer::index::get(
        &peer::index::Fields {
            parent_id: Some(root_peer.id),
            ..Default::default()
        },
        &tx,
    )?;
    let found_peers = !root_children.is_empty();

    let mut updated_peers = Vec::new();
    for peer in root_children {
        if let Some(peer) = update_peer(&peer, &mut tx)? {
            updated_peers.push(peer);
        }
    }

    if updated_peers.is_empty() {
        if found_peers {
            println!("All peers up-to-date.");
        }
    } else {
        let message = get_commit_message(&updated_peers)?;
        tx.commit(message.as_str())?;
    }

    update_remote()?;
    Ok(())
}

/// Update peer.
///
/// Return Some(peer) if updated, otherwise None.
fn update_peer(peer: &peer::Peer, tx: &mut common::StoreTransaction) -> Result<Option<peer::Peer>> {
    println!("Fetching: {}", peer.git_url.to_string());
    let update_found = peer::fs::fetch_update(&peer, tx)?;
    if !update_found {
        return Ok(None);
    }

    remove_index_peer_subtree(&peer, tx)?;
    peer::fs::merge_update(&peer, tx)?;
    let peer = merge_updated_peer_subtree(&peer, tx)?;
    Ok(Some(peer))
}

fn get_commit_message(updated_peers: &Vec<peer::Peer>) -> Result<String> {
    let mut message: String = "Updated peers:\n".to_owned();
    for peer in updated_peers {
        message.push_str(
            format!(
                "{alias} ({git_url})\n",
                alias = peer.alias,
                git_url = peer.git_url
            )
            .as_str(),
        );
    }
    Ok(message)
}

fn update_remote() -> Result<()> {
    if !common::fs::is_remote_repo_setup()? {
        println!(
            "Remote repository not specified.\n\
        Specify using `vouch config core.root-git-url <url>`.\n\
        Not pushing local changes."
        );
    }

    println!("Pushing local changes to remote repository.");
    common::fs::git_push_root()?;

    let config = crate::common::config::Config::load()?;
    if config.core.notify_vouch_public_sync {
        // TODO: Send notification to vouch servers.
        log::info!("Notifying Vouch central of public repo update.")
    }
    Ok(())
}

fn remove_index_peer_subtree(
    target_peer: &peer::Peer,
    tx: &mut common::StoreTransaction,
) -> Result<()> {
    // Remove subtree in sets of breadth first layers.
    let peers_breadth_layers = peer::index::get_breadth_first_child_peers(&target_peer, &tx)?;

    // Processing order: from leaves to starting peer (inclusive).
    for peers in peers_breadth_layers.iter().rev() {
        for peer in peers {
            review::index::remove(
                &review::index::Fields {
                    peer: Some(&peer),
                    ..Default::default()
                },
                &tx,
            )?;
            peer::index::remove(
                &peer::index::Fields {
                    id: Some(peer.id),
                    ..Default::default()
                },
                &tx,
            )?;
        }
    }
    Ok(())
}

fn merge_updated_peer_subtree(
    peer: &peer::Peer,
    tx: &mut common::StoreTransaction,
) -> Result<peer::Peer> {
    // Get an up-to-date copy of the root peer.
    let mut root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;
    let peer = peer::index::insert(&peer.alias, &peer.git_url, Some(&mut root_peer), &tx)?;

    let mut peer_store = store::Store::from_peer(&vec![root_peer.clone(), peer.clone()])?;
    let peer_index_tx = peer_store.get_transaction()?;
    store::index::merge(&peer.git_url, &peer_index_tx, &tx)?;

    Ok(peer)
}
