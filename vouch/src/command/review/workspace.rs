use anyhow::{format_err, Context, Result};
use std::io::Write;

use crate::common;
use crate::package;
use crate::review;

/// Setup review workspace.
///
/// Download and unpack package source code for review.
pub fn setup(package: &package::Package) -> Result<std::path::PathBuf> {
    let extension = get_archive_extension(&package.source_code_url)?;

    let package_unique_directory = setup_unique_package_directory(&package)?;
    let archive_path = package_unique_directory.join(format!("package.{}", extension));

    download_archive(&package.source_code_url, &archive_path)?;
    let workspace_directory = match extension.as_str() {
        "zip" => extract_zip(&archive_path, &package_unique_directory)?,
        _ => unimplemented!("Unsupported archive file type: {}", extension),
    };
    std::fs::remove_file(&archive_path)?;

    let workspace_directory = normalize_workspace_directory_name(
        &workspace_directory,
        &package_unique_directory,
        &package,
    )?;

    Ok(workspace_directory)
}

/// Extract and return archive file extension from archive URL.
fn get_archive_extension(archive_url: &url::Url) -> Result<String> {
    Ok(std::path::Path::new(archive_url.path())
        .extension()
        .ok_or(format_err!(
            "Failed to parse file extension from archive URL: {}",
            archive_url
        ))?
        .to_str()
        .ok_or(format_err!(
            "Failed to parse file extension unicode charecters."
        ))?
        .to_owned())
}

fn get_unique_package_directory(package: &package::Package) -> Result<std::path::PathBuf> {
    let data_paths = common::fs::DataPaths::new()?;
    let package_unique_directory = data_paths
        .ongoing_reviews_directory
        .join(review::fs::get_unique_package_path(&package)?);
    Ok(package_unique_directory)
}

/// Setup package version unique workspace parent directory.
fn setup_unique_package_directory(package: &package::Package) -> Result<std::path::PathBuf> {
    let package_unique_directory = get_unique_package_directory(&package)?;
    std::fs::create_dir_all(&package_unique_directory).context(format!(
        "Can't create directory: {}",
        package_unique_directory.display()
    ))?;
    Ok(package_unique_directory)
}

fn download_archive(target_url: &url::Url, destination_path: &std::path::PathBuf) -> Result<()> {
    log::debug!(
        "Downloading archive to destination path: {}",
        destination_path.display()
    );

    let response = reqwest::blocking::get(target_url.clone())?;
    let mut file = std::fs::File::create(&destination_path)?;
    let content = response.bytes()?;
    file.write_all(&content)?;

    log::debug!("Finished writing archive.");

    Ok(())
}

fn extract_zip(
    archive_path: &std::path::PathBuf,
    destination_directory: &std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    log::debug!("Extracting zip archive: {}", archive_path.display());

    let file = std::fs::File::open(&archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let extracted_directory = destination_directory.join(
        archive
            .by_index(0)?
            .enclosed_name()
            .ok_or(format_err!(
                "Archive is unexpectedly empty: {}",
                archive_path.display()
            ))?
            .to_path_buf(),
    );

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let output_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let output_path = destination_directory.join(output_path);

        if (&*file.name()).ends_with('/') {
            std::fs::create_dir_all(&output_path)?;
        } else {
            if let Some(parent) = output_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(&parent)?;
                }
            }
            let mut output_file = std::fs::File::create(&output_path)?;
            std::io::copy(&mut file, &mut output_file)?;
        }
    }
    log::debug!("Archive extraction complete.");
    Ok(extracted_directory)
}

fn get_workspace_directory_name(package: &package::Package) -> Result<std::path::PathBuf> {
    Ok(std::path::PathBuf::from(format!(
        "{}-{}",
        package.name, package.version
    )))
}

fn normalize_workspace_directory_name(
    workspace_directory: &std::path::PathBuf,
    parent_directory: &std::path::PathBuf,
    package: &package::Package,
) -> Result<std::path::PathBuf> {
    let target_directory = parent_directory.join(get_workspace_directory_name(&package)?);
    std::fs::rename(workspace_directory, &target_directory)?;
    Ok(target_directory)
}

/// Returns optional path to existing review workspace directory.
pub fn get_existing_ongoing_workspace(
    package: &package::Package,
) -> Result<Option<std::path::PathBuf>> {
    let package_unique_directory = get_unique_package_directory(&package)?;
    let workspace_directory =
        package_unique_directory.join(get_workspace_directory_name(&package)?);
    if workspace_directory.exists() {
        Ok(Some(workspace_directory))
    } else {
        Ok(None)
    }
}

pub fn run_review_tool(workspace_directory: &std::path::PathBuf) -> Result<()> {
    log::debug!("Running review tool.");

    let mut child = std::process::Command::new("code")
        .args(vec![
            "--wait",
            "--new-window",
            workspace_directory.to_str().ok_or(format_err!(
                "Failed to convert PathBuf to str: {}",
                workspace_directory.display()
            ))?,
        ])
        .current_dir(workspace_directory)
        .spawn()
        .expect("Failed to start vscode.");
    let _result = child.wait()?;

    log::debug!("Review tool exit complete.");
    Ok(())
}
