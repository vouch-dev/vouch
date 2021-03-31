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
    let peers_found = !root_children.is_empty();

    let mut updated_peers = Vec::new();
    for peer in root_children {
        println!("Fetching: {}", peer.git_url.to_string());
        let update_found = peer::fs::fetch_update(&peer, &mut tx)?;
        if !update_found {
            continue;
        }

        remove_peer_index_data(&peer, &mut tx)?;
        peer::fs::merge_update(&peer, &mut tx)?;
        let peer = merge_updated_peer_subtree(&peer, &mut tx)?;
        updated_peers.push(peer);
    }

    if updated_peers.is_empty() {
        if peers_found {
            println!("All peers up-to-date.");
        }
    } else {
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
        tx.commit(message.as_str())?;
    }

    if common::fs::is_remote_repo_setup()? {
        println!("Pushing local changes to remote repository.");
        common::fs::git_push_root()?;
    } else {
        println!(
            "Remote repository not specified.\n\
        Specify using `vouch config core.root-git-url <url>`.\n\
        Not pushing local changes."
        );
    }

    Ok(())
}

fn remove_peer_index_data(
    target_peer: &peer::Peer,
    tx: &mut common::StoreTransaction,
) -> Result<()> {
    // Remove subtree in sets of breadth first layers.
    let peers_breadth_layers = peer::index::get_breadth_first_child_peers(&target_peer, &tx)?;

    // Processing order: from leaves to root.
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
