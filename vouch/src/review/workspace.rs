use anyhow::{format_err, Context, Result};
use std::io::Write;

use crate::common;
use crate::review;

/// Ensure review workspace setup is complete.
///
/// Download and unpack package for review.
/// If ongoing workspace exists, return directory path.
pub fn ensure(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
    archive_url: &url::Url,
) -> Result<(std::path::PathBuf, Option<String>)> {
    if let Some(workspace_directory) =
        get_existing(&package_name, &package_version, &registry_host_name)?
    {
        return Ok((workspace_directory, None));
    }

    let file_extension = get_archive_file_extension(&archive_url)?;

    let package_unique_directory =
        setup_unique_package_directory(&package_name, &package_version, &registry_host_name)?;
    let archive_path = package_unique_directory.join(format!("package.{}", file_extension));

    download_archive(&archive_url, &archive_path)?;
    let (archive_hash, _) = common::fs::hash(&archive_path)?;

    log::debug!("Extracting archive: {}", archive_path.display());
    let workspace_directory = match file_extension.as_str() {
        "zip" => extract_zip(&archive_path, &package_unique_directory)?,
        "tgz" | "tar.gz" => extract_tar_gz(&archive_path, &package_unique_directory)?,
        _ => unimplemented!("Unsupported archive file type: {}", file_extension),
    };
    log::debug!("Archive extraction complete.");
    std::fs::remove_file(&archive_path)?;

    let workspace_directory = normalize_workspace_directory_name(
        &workspace_directory,
        &package_unique_directory,
        &package_name,
        &package_version,
    )?;

    Ok((workspace_directory, Some(archive_hash)))
}

/// Returns optional path to existing review workspace directory.
pub fn get_existing(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
) -> Result<Option<std::path::PathBuf>> {
    let package_unique_directory =
        get_unique_package_directory(&package_name, &package_version, &registry_host_name)?;
    let workspace_directory = package_unique_directory.join(get_workspace_directory_name(
        &package_name,
        &package_version,
    )?);
    if workspace_directory.exists() {
        Ok(Some(workspace_directory))
    } else {
        Ok(None)
    }
}

/// Extract and return archive file extension from archive URL.
fn get_archive_file_extension(archive_url: &url::Url) -> Result<String> {
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

fn get_unique_package_directory(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
) -> Result<std::path::PathBuf> {
    let data_paths = common::fs::DataPaths::new()?;
    let package_unique_directory =
        data_paths
            .ongoing_reviews_directory
            .join(review::fs::get_unique_package_path(
                &package_name,
                &package_version,
                &registry_host_name,
            )?);
    Ok(package_unique_directory)
}

/// Setup package version unique workspace parent directory.
fn setup_unique_package_directory(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
) -> Result<std::path::PathBuf> {
    let package_unique_directory =
        get_unique_package_directory(&package_name, &package_version, &registry_host_name)?;
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

fn get_workspace_directory_name(
    package_name: &str,
    package_version: &str,
) -> Result<std::path::PathBuf> {
    Ok(std::path::PathBuf::from(format!(
        "{}-{}",
        package_name, package_version
    )))
}

fn normalize_workspace_directory_name(
    workspace_directory: &std::path::PathBuf,
    parent_directory: &std::path::PathBuf,
    package_name: &str,
    package_version: &str,
) -> Result<std::path::PathBuf> {
    let target_directory = parent_directory.join(get_workspace_directory_name(
        &package_name,
        &package_version,
    )?);
    std::fs::rename(workspace_directory, &target_directory)?;
    Ok(target_directory)
}

/// Analyse workspace file line counts.
fn get_file_line_counts(
    workspace_directory: &std::path::PathBuf,
) -> Result<std::collections::BTreeMap<std::path::PathBuf, usize>> {
    let paths = &[workspace_directory];
    let excluded = &[];
    let config = tokei::Config {
        hidden: Some(true),
        no_ignore: Some(true),
        ..tokei::Config::default()
    };
    let mut languages = tokei::Languages::new();
    languages.get_statistics(paths, excluded, &config);

    let mut file_line_counts = std::collections::BTreeMap::new();

    for (_language_type, langauge) in &languages {
        for report in &langauge.reports {
            let file_path = report.name.clone();
            let total_line_count = report.stats.lines();
            *file_line_counts.entry(file_path).or_insert(0) += total_line_count;
        }
    }
    Ok(file_line_counts)
}

/// Sum directory line counts from file line counts.
fn get_directory_line_counts(
    file_line_counts: &std::collections::BTreeMap<std::path::PathBuf, usize>,
    workspace_directory: &std::path::PathBuf,
) -> Result<std::collections::BTreeMap<std::path::PathBuf, usize>> {
    let mut directory_line_counts = std::collections::BTreeMap::new();
    for (file_path, line_count) in file_line_counts.iter() {
        let mut path = file_path.clone();
        while path.pop() {
            *directory_line_counts.entry(path.clone()).or_insert(0) += line_count;
            if path == *workspace_directory {
                break;
            }
        }
    }
    Ok(directory_line_counts.clone())
}

#[derive(Debug)]
pub struct PathAnalysis {
    pub path_type: common::fs::PathType,
    pub line_count: usize,
}

pub type Analysis = std::collections::BTreeMap<std::path::PathBuf, PathAnalysis>;

/// Analyse workspace line counts.
pub fn analyse(workspace_directory: &std::path::PathBuf) -> Result<Analysis> {
    let file_line_counts = get_file_line_counts(&workspace_directory)?;
    let directory_line_counts = get_directory_line_counts(&file_line_counts, &workspace_directory)?;

    let mut analysis = std::collections::BTreeMap::new();
    for (path_type, line_counts) in vec![
        (common::fs::PathType::File, file_line_counts),
        (common::fs::PathType::Directory, directory_line_counts),
    ] {
        for (path, line_count) in line_counts.into_iter() {
            let path = path.strip_prefix(workspace_directory)?.to_path_buf();
            analysis.insert(
                path,
                PathAnalysis {
                    path_type,
                    line_count,
                },
            );
        }
    }
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_extension_extracted_for_tar_gz() -> Result<()> {
        let result =
            get_archive_file_extension(&url::Url::parse("https://localhost/d3/d3-4.10.0.tar.gz")?)?;
        let expected = "tar.gz".to_string();
        assert!(result == expected, format!("unexpected result: {}", result));
        Ok(())
    }

    #[test]
    fn test_correct_directory_line_counts() -> Result<()> {
        let workspace_directory = std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0");
        let file_line_counts = maplit::btreemap! {
            std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0/file_1.js") => 22,
            std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0/build/file_2.js") => 37,
            std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0/build/file_3.js") => 5,
        };

        let result = get_directory_line_counts(&file_line_counts, &workspace_directory)?;
        let expected = maplit::btreemap! {
            std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0") => 64,
            std::path::PathBuf::from("/npmjs.com/d3/4.10.0/d3-4.10.0/build") => 42,
        };
        assert_eq!(result, expected);
        Ok(())
    }
}

pub fn remove(workspace: &std::path::PathBuf) -> Result<()> {
    log::debug!("Removing workspace directory: {}", workspace.display());
    std::fs::remove_dir_all(&workspace)?;
    let paths = common::fs::DataPaths::new()?;
    common::fs::remove_empty_directories(&workspace, &paths.ongoing_reviews_directory)?;
    Ok(())
}
