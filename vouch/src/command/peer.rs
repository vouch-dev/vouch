use anyhow::{format_err, Result};
use std::convert::TryFrom;
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
pub struct AddArguments {
    /// Git repository URL.
    #[structopt(name = "git-url", parse(try_from_str = crate::common::GitUrl::try_from))]
    pub git_url: crate::common::GitUrl,
}

pub fn add(args: &AddArguments) -> Result<()> {
    let mut store = store::Store::from_root()?;
    let mut tx = store.get_transaction()?;

    let mut root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;

    if let Some(peer) = peer::index::get(
        &peer::index::Fields {
            git_url: Some(&args.git_url),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    {
        let parent_id = peer.parent_id.ok_or(format_err!(
            "The given git URL is already assigned the root peer."
        ))?;

        if parent_id == root_peer.id {
            // Peer exist in the index and has root as parent.
            // Peer can't move further up the peer tree.
            // Nothing more to do.
            return Ok(());
        }

        remove_peer_subtree(&peer, &mut tx)?;
    }

    peer::fs::add(&args.git_url, &mut tx)?;

    let alias = peer::index::get_new_alias(&args.git_url, &mut tx)?;
    let peer = peer::index::insert(&alias, &args.git_url, Some(&mut root_peer), &tx)?;
    let mut peer_store = store::Store::from_peer(&vec![root_peer, peer])?;
    let peer_index_tx = peer_store.get_transaction()?;
    store::index::merge(&args.git_url, &peer_index_tx, &tx)?;

    tx.commit(format!("Follow: {}", &args.git_url).as_str())?;
    Ok(())
}

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct RemoveArguments {
    /// Git repository URL.
    #[structopt(name = "git-url", parse(try_from_str = crate::common::GitUrl::try_from))]
    pub git_url: crate::common::GitUrl,
}

pub fn remove(args: &RemoveArguments) -> Result<()> {
    let mut store = store::Store::from_root()?;
    let mut tx = store.get_transaction()?;

    let target_peer = peer::index::get(
        &peer::index::Fields {
            git_url: Some(&args.git_url),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    .ok_or(format_err!(
        "Failed to find peer for removal: {}",
        &args.git_url
    ))?;

    remove_peer_subtree(&target_peer, &mut tx)?;

    tx.commit(
        format!(
            "Unfollow: {alias} ({git_url})",
            alias = target_peer.alias,
            git_url = target_peer.git_url
        )
        .as_str(),
    )?;
    Ok(())
}

fn remove_peer_subtree(peer: &peer::Peer, tx: &mut common::StoreTransaction) -> Result<()> {
    let root_to_peer_subtree = peer::index::get_root_to_peer_subtree(&peer, &tx)?;
    peer::fs::remove(&root_to_peer_subtree, tx)?;

    // Remove subtree in sets of breadth first layers.
    // Processing order: from leaves to root.
    let peers_breadth_layers = peer::index::get_breadth_first_child_peers(&peer, &tx)?;
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
