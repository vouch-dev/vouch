use anyhow::{format_err, Context, Result};
use serde_yaml;
use std::io::Write;

use crate::common;
use crate::review;

pub type StrFinalizeCallback = fn(&str) -> Result<String>;

static SUMMARY_FILE_NAME: &str = "summary.yaml";

/// Given a review summary, returs a review specific relative directory path.
///
/// Example: "pypi.org/numpy/1.18.5"
fn get_review_directory_path(review: &review::Review) -> Result<std::path::PathBuf> {
    let registry_host_name = std::path::PathBuf::from(&review.package.registry.host_name);
    Ok(registry_host_name
        .join(&review.package.name)
        .join(&review.package.version))
}

fn get_storage_file_path(review: &review::Review) -> Result<std::path::PathBuf> {
    let review_directory_path = get_review_directory_path(&review)?;

    let paths = common::fs::DataPaths::new()?;
    let package_specific_directory = paths.reviews_directory.join(review_directory_path);
    Ok(package_specific_directory.join(SUMMARY_FILE_NAME))
}

/// Store a review summary.
pub fn add(
    review: &review::Review,
    str_finalize_callback: Option<StrFinalizeCallback>,
) -> Result<()> {
    // Convert review to string and pass through callback. This provides an
    // opportunity to add comments.
    let mut review_string = serde_yaml::to_string(&review)?;
    review_string = match str_finalize_callback {
        Some(callback) => callback(&review_string)?,
        None => review_string,
    };

    let file_path = get_storage_file_path(&review)?;
    let parent_directory = file_path.parent().ok_or(format_err!(
        "Can't find parent directory for file path: {}",
        file_path.display()
    ))?;
    std::fs::create_dir_all(&parent_directory).context(format!(
        "Can't create directory: {}",
        parent_directory.display()
    ))?;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&file_path)
        .context(format!(
            "Can't open/create file for writing: {}",
            file_path.display()
        ))?;
    file.write_all(review_string.as_bytes())
        .expect("Unable to write review to file.");
    Ok(())
}
