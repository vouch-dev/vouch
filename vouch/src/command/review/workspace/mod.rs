use anyhow::{format_err, Context, Result};
use std::io::Write;

use crate::common;
use crate::package;
use crate::review;

mod vscode;

/// Setup review workspace.
///
/// Download and unpack package source code for review.
pub fn setup(package: &package::Package) -> Result<std::path::PathBuf> {
    let extension = get_archive_extension(&package.source_code_url)?;

    let package_unique_directory = setup_unique_package_directory(&package)?;
    let archive_path = package_unique_directory.join(format!("package.{}", extension));

    download_archive(&package.source_code_url, &archive_path)?;

    log::debug!("Extracting archive: {}", archive_path.display());
    let workspace_directory = match extension.as_str() {
        "zip" => extract_zip(&archive_path, &package_unique_directory)?,
        "tgz" | "tar.gz" => extract_tar_gz(&archive_path, &package_unique_directory)?,
        _ => unimplemented!("Unsupported archive file type: {}", extension),
    };
    log::debug!("Archive extraction complete.");
    std::fs::remove_file(&archive_path)?;

    let workspace_directory = normalize_workspace_directory_name(
        &workspace_directory,
        &package_unique_directory,
        &package,
    )?;

    let reviews_directory = vscode::setup(&workspace_directory)?;
    add_empty_review(&package, &reviews_directory)?;

    Ok(workspace_directory)
}

fn add_empty_review(
    package: &package::Package,
    reviews_directory: &std::path::PathBuf,
) -> Result<()> {
    let detailed_review = review::DetailedReview {
        title: "local".to_string(),
        description: format!("Package name-version: {}-{}", package.name, package.version),
        is_primary: Some(true),
        comments: Vec::new(),
    };

    let review_file_path = reviews_directory.join("local.review");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&review_file_path)
        .context(format!(
            "Can't open/create file for writing: {}",
            review_file_path.display()
        ))?;
    file.write_all(serde_json::to_string_pretty(&detailed_review)?.as_bytes())?;
    Ok(())
}

/// Extract and return archive file extension from archive URL.
fn get_archive_extension(archive_url: &url::Url) -> Result<String> {
    let path = std::path::Path::new(archive_url.path());
    if path
        .to_str()
        .ok_or(format_err!("Failed to parse URL path as str."))?
        .ends_with(".tar.gz")
    {
        return Ok("tar.gz".to_string());
    }

    Ok(path
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
    Ok(extracted_directory)
}

/// Extract .tar.gz archives.
///
/// Note that .tgz archives are the same as .tar.gz archives.
fn extract_tar_gz(
    archive_path: &std::path::PathBuf,
    destination_directory: &std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    let top_directory_name = get_tar_top_directory_name(&archive_path)?;
    log::debug!(
        "Found archive top level directory name: {}",
        top_directory_name
    );

    let file = std::fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let workspace_directory = destination_directory.join(top_directory_name);

    archive.unpack(&destination_directory)?;
    Ok(workspace_directory)
}

/// Returns the top level directory name from within the given archive.
///
/// This function advances the archive's position counter.
/// The archive can not be unpacked after this operation, it is therefore dropped.
fn get_tar_top_directory_name(archive_path: &std::path::PathBuf) -> Result<String> {
    let file = std::fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let first_archive_entry = archive
        .entries()?
        .nth(0)
        .ok_or(format_err!("Archive empty."))??;
    let first_archive_entry = (*first_archive_entry.path()?).to_path_buf();

    let top_directory_name = first_archive_entry
        .components()
        .next()
        .ok_or(format_err!("Archive empty."))?
        .as_os_str()
        .to_str()
        .ok_or(format_err!("Failed to parse archive's first path."))?;

    Ok(top_directory_name.to_string())
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
    // TODO: Check vscode installed and vscode-vouch extension installed.
    vscode::run(&workspace_directory)?;
    log::debug!("Review tool exit complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blah() -> Result<()> {
        let result =
            get_archive_extension(&url::Url::parse("https://localhost/d3/d3-4.10.0.tar.gz")?)?;
        let expected = "tar.gz".to_string();
        assert!(result == expected, format!("unexpected result: {}", result));
        Ok(())
    }
}
