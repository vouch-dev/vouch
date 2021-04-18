use anyhow::{format_err, Result};
use directories;

pub mod archive;

/// Filesystem vouch config directory absolute paths.
#[derive(Debug)]
pub struct ConfigPaths {
    pub root_directory: std::path::PathBuf,
    pub config_file: std::path::PathBuf,
    pub extensions_directory: std::path::PathBuf,
}

impl ConfigPaths {
    pub fn new() -> Result<Self> {
        let user_directories = directories::ProjectDirs::from("", "", "vouch").ok_or(
            format_err!("Failed to obtain a handle on the local user directory."),
        )?;
        let root_directory = user_directories.config_dir();
        Ok(Self {
            root_directory: root_directory.into(),
            config_file: root_directory.join("config.yaml"),
            extensions_directory: root_directory.join("extensions"),
        })
    }
}

/// Filesystem vouch data directory absolute paths.
#[derive(Debug)]
pub struct DataPaths {
    pub root_directory: std::path::PathBuf,

    pub index_directory: std::path::PathBuf,
    pub index_file: std::path::PathBuf,

    pub reviews_directory: std::path::PathBuf,
    pub ongoing_reviews_directory: std::path::PathBuf,

    pub peers_directory: std::path::PathBuf,
}

impl DataPaths {
    pub fn from_root_directory(root_directory: &std::path::PathBuf) -> Result<Self> {
        Ok(Self {
            root_directory: root_directory.clone(),

            index_directory: root_directory.join(".index"),
            index_file: root_directory.join(".index").join("index.db"),

            reviews_directory: root_directory.join("reviews"),
            ongoing_reviews_directory: root_directory.join("reviews").join(".ongoing"),

            peers_directory: root_directory.join("peers"),
        })
    }

    pub fn new() -> Result<Self> {
        let user_directories = directories::ProjectDirs::from("", "", "vouch").ok_or(
            format_err!("Failed to obtain a handle on the local user directory."),
        )?;
        let root_directory = user_directories.data_local_dir();
        Self::from_root_directory(&root_directory.into())
    }

    /// Returns true if the given absolute path is protected from deletion, otherwise false.
    pub fn is_protected(&self, absolute_path: &std::path::PathBuf) -> bool {
        absolute_path == &self.root_directory
            || absolute_path == &self.index_directory
            || absolute_path == &self.reviews_directory
            || absolute_path == &self.ongoing_reviews_directory
            || absolute_path == &self.peers_directory
    }
}

pub fn git(args: Vec<&str>, working_directory: &std::path::PathBuf) -> Result<()> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(working_directory)
        .status()?;
    Ok(())
}

/// Remove empty directories along relative path.
pub fn remove_empty_directories(
    relative_path: &std::path::PathBuf,
    working_directory: &std::path::PathBuf,
) -> Result<()> {
    let paths = DataPaths::new()?;

    let mut absolute_path = working_directory.join(relative_path);
    while &absolute_path != working_directory {
        if paths.is_protected(&absolute_path) {
            break;
        }
        if !absolute_path.exists() {
            absolute_path.pop();
            continue;
        }
        if std::fs::remove_dir(&absolute_path).is_err() {
            // Found first non-empty directory.
            break;
        }
        absolute_path.pop();
    }
    Ok(())
}

/// Remove a git submodule.
///
/// See: https://stackoverflow.com/a/36593218/1339591
pub fn git_remove_submodule(
    submodule_relative_path: &std::path::PathBuf,
    root_directory: &std::path::PathBuf,
) -> Result<()> {
    // Make a str version because its used throughout.
    let submodule_relative_path_str = submodule_relative_path.to_str().ok_or(format_err!(
        "Could not parse submodule path: {}",
        submodule_relative_path.display()
    ))?;

    // Remove the submodule entry from .git/config
    std::process::Command::new("git")
        .args(vec![
            "submodule",
            "deinit",
            "-f",
            submodule_relative_path_str,
        ])
        .current_dir(&root_directory)
        .status()?;

    // // Remove the submodule directory from the superproject's .git/modules directory
    let modules_path = format!(".git/modules/{}", submodule_relative_path_str);
    std::fs::remove_dir_all(&root_directory.join(modules_path))?;
    remove_empty_directories(
        &submodule_relative_path,
        &root_directory.join(".git/modules"),
    )?;

    // Remove the entry in .gitmodules and remove the submodule directory
    std::process::Command::new("git")
        .args(vec!["rm", "-f", submodule_relative_path_str])
        .current_dir(&root_directory)
        .status()?;
    remove_empty_directories(&submodule_relative_path, &root_directory)?;

    Ok(())
}

/// Deinit a git submodule.
pub fn git_deinit_submodule(
    submodule_path: &std::path::PathBuf,
    working_directory: &std::path::PathBuf,
) -> Result<()> {
    let submodule_path = submodule_path.to_str().ok_or(format_err!(
        "Could not parse submodule path: {}",
        submodule_path.display()
    ))?;

    std::process::Command::new("git")
        .args(vec!["submodule", "deinit", "-f", submodule_path])
        .current_dir(&working_directory)
        .status()?;

    Ok(())
}

pub fn is_remote_repo_setup() -> Result<bool> {
    let paths = DataPaths::new()?;
    let repo = git2::Repository::open(&paths.root_directory)?;
    Ok(!repo.remotes()?.is_empty())
}

pub fn git_push_root() -> Result<()> {
    let paths = DataPaths::new()?;
    git(
        vec!["push", "--set-upstream", "origin", "master"],
        &paths.root_directory,
    )?;
    Ok(())
}

pub struct GitTransaction {
    working_directory: std::path::PathBuf,
}

impl GitTransaction {
    pub fn new() -> Result<Self> {
        let paths = DataPaths::new()?;

        // TODO: Get initial commit for atomic reversion.
        // let repository = git2::Repository::open(&paths.root_data_directory)?;
        // let head_reference = repository.head()?;
        // let initial_commit_hash = head_reference.peel_to_commit()?.id();

        Ok(Self {
            working_directory: paths.root_directory.clone(),
        })
    }

    pub fn commit(&mut self, message: &str) -> Result<()> {
        let args = vec!["add", "-A"];
        git(args, &self.working_directory)?;

        let args = vec!["commit", "-am", message];
        git(args, &self.working_directory)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PathType {
    File,
    Directory,
}

fn blake3_digest<R: std::io::Read>(mut reader: R) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hasher.finalize().to_hex().as_str().to_string())
}

fn hash_file(path: &std::path::PathBuf) -> Result<String> {
    let input = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(input);
    Ok(blake3_digest(reader)?)
}

pub fn hash(path: &std::path::PathBuf) -> Result<(String, PathType)> {
    if path.is_file() {
        return Ok((hash_file(&path)?, PathType::File));
    } else {
        unimplemented!("Only file hashing is currently implemented.");
    }
}
