use anyhow::{format_err, Result};
use git2;
use std::io::Write;

use crate::common;
use crate::extension;

fn handle_nonempty_git_repository(directory_path: &std::path::PathBuf, force: bool) -> Result<()> {
    let target_directory_empty = directory_path.read_dir()?.next().is_none();
    if !force && !target_directory_empty {
        // TODO: Check with storage::sync for unsynchronized changes. Improve feedback.
        return Err(format_err!(
            "Setup directory is not empty ({}).\nUse --force to overwrite existing data.",
            &directory_path.display()
        ));
    }
    if force && !target_directory_empty {
        // Delete directory contents so that git clone can succeed.
        std::fs::remove_dir_all(&directory_path)?;
        std::fs::create_dir_all(&directory_path)?;
    }
    Ok(())
}

/// Adds a pattern to the .git/info/exclude file if absent.
fn append_git_exclude(exclusion_pattern: &str, git_root_path: &std::path::PathBuf) -> Result<()> {
    let exclude_file_path = git_root_path.join(".git").join("info").join("exclude");

    // Check to see whether pattern is already included in file.
    let contents = std::fs::read_to_string(&exclude_file_path)?;
    let mut pattern_found = false;
    for line in contents.split("\n") {
        if line == exclusion_pattern {
            pattern_found = true;
            break;
        }
    }

    if !pattern_found {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&exclude_file_path)?;
        file.write(exclusion_pattern.as_bytes())?;
    }
    Ok(())
}

fn setup_git_repository(
    remote_repository_url: &Option<common::GitUrl>,
    paths: &common::fs::DataPaths,
    force: bool,
) -> Result<()> {
    handle_nonempty_git_repository(&paths.root_directory, force)?;

    if let Some(remote_repository_url) = remote_repository_url {
        log::debug!(
            "Cloning git repository from: {}",
            remote_repository_url.to_string()
        );
        common::fs::git(
            vec![
                "clone",
                remote_repository_url.to_string().as_str(),
                &paths.root_directory.to_str().unwrap(),
            ],
            &paths
                .root_directory
                .parent()
                .unwrap_or(&std::path::PathBuf::from(r"/"))
                .to_path_buf(),
        )?;
    } else {
        log::debug!("Initialising git repository.");
        git2::Repository::init(&paths.root_directory)?;
    }
    Ok(())
}

fn setup_data_directory_contents(paths: &common::fs::DataPaths) -> Result<()> {
    std::fs::create_dir_all(&paths.index_directory)?;
    std::fs::File::create(&paths.index_directory.join(".gitkeep"))?;

    std::fs::create_dir_all(&paths.reviews_directory)?;
    std::fs::File::create(&paths.reviews_directory.join(".gitkeep"))?;

    std::fs::create_dir_all(&paths.ongoing_reviews_directory)?;
    std::fs::File::create(&paths.ongoing_reviews_directory.join(".gitkeep"))?;
    append_git_exclude("reviews/.ongoing", &paths.root_directory)?;

    std::fs::create_dir_all(&paths.peers_directory)?;
    std::fs::File::create(&paths.peers_directory.join(".gitkeep"))?;

    // TODO: Populate README.md with reasonable message, stats, links.
    let readme_file_path = paths.root_directory.join("README.md");
    if !readme_file_path.is_file() {
        std::fs::File::create(&readme_file_path)?;
    }
    Ok(())
}

/// Setup config file.
///
/// If config file exists and force is false, file will not be modified.
fn setup_config(
    remote_repository_url: &Option<common::GitUrl>,
    paths: &common::fs::ConfigPaths,
    force: bool,
) -> Result<()> {
    std::fs::create_dir_all(&paths.root_directory)?;
    if force || !paths.config_file.is_file() {
        log::debug!("Generating config file: {}", paths.config_file.display());
        let mut config = crate::common::config::Config::default();

        config.core.root_git_url = remote_repository_url.clone();
        config.review_tool.name = "vscode".to_string();
        config.review_tool.install_check = false;

        for extension in extension::get_all_extensions()? {
            config.extensions.enabled.insert(extension.name(), true);

            for registry in extension.registries() {
                config
                    .extensions
                    .supported_package_registries
                    .insert(registry, extension.name());
            }
        }

        config.dump()?;
    } else {
        log::debug!(
            "Not overwriting existing config file (--force: {:?}): {}",
            force,
            paths.config_file.display()
        );
    }
    Ok(())
}

pub fn setup(remote_repository_url: &Option<common::GitUrl>, force: bool) -> Result<()> {
    let data_paths = common::fs::DataPaths::new()?;
    log::debug!("Using data paths: {:#?}", data_paths);

    let config_paths = common::fs::ConfigPaths::new()?;
    log::debug!("Using config paths: {:#?}", config_paths);
    setup_config(&remote_repository_url, &config_paths, force)?;
    log::debug!("Config setup complete.");

    log::debug!("Ensuring root data directory exists.");
    std::fs::create_dir_all(&data_paths.root_directory)?;

    setup_git_repository(&remote_repository_url, &data_paths, force)?;
    log::debug!("Repo git setup complete.");

    setup_data_directory_contents(&data_paths)?;

    Ok(())
}

/// Returns true if setup is complete, otherwise returns false.
///
/// Checks for existance of config file and for reviews directory.
pub fn is_complete() -> Result<bool> {
    let config_paths = common::fs::ConfigPaths::new()?;
    let data_paths = common::fs::DataPaths::new()?;
    Ok(config_paths.config_file.is_file() && data_paths.reviews_directory.is_dir())
}
