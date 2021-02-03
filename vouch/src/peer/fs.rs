use anyhow::{format_err, Result};

use super::common;
use crate::common::{fs::DataPaths, StoreTransaction};

/// For a given Git repository URL, returns the target submodule path.
fn get_submodule_storage_relative_path(
    git_url: &crate::common::GitUrl,
) -> Result<std::path::PathBuf> {
    let url_registry_component = git_url.url().host_str().ok_or(format_err!(
        "Cannot derive registry from URL: {:?}",
        git_url
    ))?;

    let url_path_component = std::path::PathBuf::from(git_url.url().path());
    let url_path_component = url_path_component.strip_prefix("/")?;

    Ok(std::path::PathBuf::from(url_registry_component).join(url_path_component))
}

/// Add a peer as a child of the root peer via Git repository URL.
pub fn add(git_url: &crate::common::GitUrl, _tx: &mut StoreTransaction) -> Result<()> {
    let paths = DataPaths::new()?;

    let submodule_relative_path = get_submodule_storage_relative_path(git_url)?;
    let peers_directory_name = paths.peers_directory.strip_prefix(&paths.root_directory)?;
    let submodule_relative_path = peers_directory_name.join(submodule_relative_path);

    let args = vec![
        "submodule",
        "add",
        git_url.as_str(),
        submodule_relative_path.to_str().ok_or(format_err!(
            "Could not parse submodule path: {:?}",
            submodule_relative_path
        ))?,
    ];
    crate::common::fs::git(args, &paths.root_directory)?;
    Ok(())
}

pub fn get_root_database() -> Result<rusqlite::Connection> {
    let paths = DataPaths::new()?;
    Ok(rusqlite::Connection::open(paths.index_file)?)
}

pub fn get_peer_database(peer_subtree: &Vec<common::Peer>) -> Result<rusqlite::Connection> {
    let root_peer_paths = DataPaths::new()?;
    let peer_path = get_peer_path(&peer_subtree, &root_peer_paths.root_directory)?;
    let paths = DataPaths::from_root_directory(&peer_path)?;
    Ok(rusqlite::Connection::open(paths.index_file)?)
}

pub fn remove(peer_subtree: &Vec<common::Peer>, tx: &mut StoreTransaction) -> Result<()> {
    assert!(
        peer_subtree.len() >= 2,
        "Cannot remove root peer. First element in subtree must be root peer."
    );

    let peer = peer_subtree
        .last()
        .ok_or(format_err!("invalid peer_subtree"))?;
    let imediate_parent_peer = peer_subtree
        .get(peer_subtree.len() - 2)
        .ok_or(format_err!("invalid peer_subtree"))?;

    if imediate_parent_peer.is_root() {
        remove_direct_follow(&peer, tx)?;
    } else {
        remove_indirect_follow(&peer_subtree)?;
    }

    Ok(())
}

/// Given a directly followed peer, fetches from origin/master.
/// Returns true if a remote update is available.
///
/// See: https://stackoverflow.com/questions/58768910/how-to-perform-git-pull-with-the-rust-git2-crate
pub fn fetch_update(peer: &common::Peer, _tx: &mut StoreTransaction) -> Result<bool> {
    let paths = DataPaths::new()?;

    let submodule_relative_path = get_submodule_storage_relative_path(&peer.git_url)?;
    let peer_submodule_path = paths.peers_directory.join(&submodule_relative_path);

    let repo = git2::Repository::open(&peer_submodule_path)?;
    // TODO: Add git2 credentials handling.
    // repo.find_remote("origin")?.fetch(&["master"], None, None)?;
    crate::common::fs::git(vec!["fetch"], &peer_submodule_path)?;

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let analysis = repo.merge_analysis(&[&fetch_commit])?;
    Ok(!analysis.0.is_up_to_date())
}

pub fn merge_update(peer: &common::Peer, _tx: &mut StoreTransaction) -> Result<()> {
    let paths = DataPaths::new()?;

    let submodule_relative_path = get_submodule_storage_relative_path(&peer.git_url)?;
    let peer_submodule_path = paths.peers_directory.join(&submodule_relative_path);

    let repo = git2::Repository::open(&peer_submodule_path)?;

    // Dont re-fetch incase the fetch has changed since first discovering a new available update.
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    assert!(
        !analysis.0.is_up_to_date(),
        "This function should only be called if fetch has indicated that an update is available."
    );

    if analysis.0.is_fast_forward() {
        let refname = "refs/heads/master";
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    } else {
        return Err(format_err!(
            "Peer update cannot be merged using fast forward: {:?}",
            peer
        ));
    }
    Ok(())
}

fn remove_direct_follow(peer: &common::Peer, _tx: &mut StoreTransaction) -> Result<()> {
    let paths = DataPaths::new()?;

    let submodule_relative_path = get_submodule_storage_relative_path(&peer.git_url)?;
    let peers_directory_name = paths.peers_directory.strip_prefix(&paths.root_directory)?;
    let submodule_relative_path = peers_directory_name.join(submodule_relative_path);

    crate::common::fs::git_remove_submodule(&submodule_relative_path, &paths.root_directory)?;
    Ok(())
}

fn remove_indirect_follow(peer_subtree: &Vec<common::Peer>) -> Result<()> {
    let paths = DataPaths::new()?;
    let peer_path = get_peer_path(&peer_subtree, &paths.root_directory)?;

    let imediate_parent_subtree = peer_subtree[..=peer_subtree.len() - 2].into();
    let imediate_parent_peer_path = get_peer_path(&imediate_parent_subtree, &paths.root_directory)?;

    crate::common::fs::git_deinit_submodule(&peer_path, &imediate_parent_peer_path)?;
    Ok(())
}

pub fn get_peer_path(
    peer_subtree: &Vec<common::Peer>,
    root_directory: &std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    let mut peer_path = std::path::PathBuf::from("/");
    for peer in peer_subtree {
        if peer.is_root() {
            peer_path = peer_path.join(&root_directory);
        } else {
            let relative_peer_directory = get_submodule_storage_relative_path(&peer.git_url)?;
            peer_path = DataPaths::from_root_directory(&peer_path)?
                .peers_directory
                .join(relative_peer_directory);
        }
    }
    Ok(peer_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_three_peer_subtree_derived_correct_path() -> Result<()> {
        let peer_subtree = vec![
            common::Peer {
                alias: "root".to_string(),
                git_url: crate::common::GitUrl::try_from("https://github.com/user_1/reviews_1")?,
                ..Default::default()
            },
            common::Peer {
                alias: "user_2".to_string(),
                git_url: crate::common::GitUrl::try_from("https://github.com/user_2/reviews_2")?,
                ..Default::default()
            },
            common::Peer {
                alias: "user_3".to_string(),
                git_url: crate::common::GitUrl::try_from("https://gitlab.com/user_3/reviews_3")?,
                ..Default::default()
            },
        ];
        let root_directory = std::path::PathBuf::from("/vouch");
        let result = get_peer_path(&peer_subtree, &root_directory)?;
        let expected = std::path::PathBuf::from(
            "/vouch/peers/github.com/user_2/reviews_2/peers/gitlab.com/user_3/reviews_3",
        );
        assert_eq!(result, expected);
        Ok(())
    }
}
