use anyhow::{format_err, Context, Result};
use std::io::Write;

use crate::common;
use crate::package;
use crate::review;

static REVIEW_FILE_NAME: &str = "review.json";

/// Given a package, returns a package version specific relative directory path.
///
/// Example: "pypi.org/numpy/1.18.5"
pub fn get_unique_package_path(package: &package::Package) -> Result<std::path::PathBuf> {
    let registry_host_name = std::path::PathBuf::from(&package.registry.host_name);
    Ok(registry_host_name
        .join(&package.name)
        .join(&package.version))
}

fn get_storage_file_path(review: &review::Review) -> Result<std::path::PathBuf> {
    let review_directory_path = get_unique_package_path(&review.package)?;

    let paths = common::fs::DataPaths::new()?;
    let package_specific_directory = paths.reviews_directory.join(review_directory_path);
    Ok(package_specific_directory.join(REVIEW_FILE_NAME))
}

/// Store a review.
pub fn add(review: &review::Review) -> Result<()> {
    let file_path = get_storage_file_path(&review)?;
    let parent_directory = file_path.parent().ok_or(format_err!(
        "Can't find parent directory for file path: {}",
        file_path.display()
    ))?;
    std::fs::create_dir_all(&parent_directory).context(format!(
        "Can't create directory: {}",
        parent_directory.display()
    ))?;

    if file_path.is_file() {
        std::fs::remove_file(&file_path)?;
    }

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&file_path)
        .context(format!(
            "Can't open/create file for writing: {}",
            file_path.display()
        ))?;
    file.write_all(serde_json::to_string_pretty(&review)?.as_bytes())?;
    Ok(())
}
