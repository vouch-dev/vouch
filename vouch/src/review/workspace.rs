use anyhow::{format_err, Context, Result};
use std::io::Write;

use crate::common;
use crate::review;

static MANIFEST_FILE_NAME: &str = "manifest.json";

// TODO: Make paths relative.
#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct Manifest {
    pub workspace_path: std::path::PathBuf,
    pub manifest_path: std::path::PathBuf,
    pub artifact_path: std::path::PathBuf,
    pub artifact_hash: String,
}

/// Ensure review workspace setup is complete.
///
/// Download and unpack package for review.
/// If ongoing workspace exists, return manifest.
pub fn ensure(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
    artifact_url: &url::Url,
) -> Result<Manifest> {
    if let Some(workspace_manifest) =
        get_existing(&package_name, &package_version, &registry_host_name)?
    {
        return Ok(workspace_manifest);
    }

    let file_extension = get_archive_file_extension(&artifact_url)?;

    let package_unique_directory =
        setup_unique_package_directory(&package_name, &package_version, &registry_host_name)?;
    let archive_path = package_unique_directory.join(format!("package.{}", file_extension));

    download_archive(&artifact_url, &archive_path)?;
    let (artifact_hash, _) = common::fs::hash(&archive_path)?;

    log::debug!("Extracting archive: {}", archive_path.display());
    let workspace_directory = match file_extension.as_str() {
        "zip" => common::fs::archive::extract_zip(&archive_path, &package_unique_directory)?,
        "tgz" | "tar.gz" => {
            common::fs::archive::extract_tar_gz(&archive_path, &package_unique_directory)?
        }
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

    let workspace_manifest = Manifest {
        workspace_path: workspace_directory,
        manifest_path: get_manifest_path(&package_unique_directory),
        artifact_path: archive_path,
        artifact_hash: artifact_hash,
    };
    write_manifest(&workspace_manifest)?;
    Ok(workspace_manifest)
}

fn get_manifest_path(package_unique_directory: &std::path::PathBuf) -> std::path::PathBuf {
    package_unique_directory.join(MANIFEST_FILE_NAME)
}

fn write_manifest(workspace_manifest: &Manifest) -> Result<()> {
    let path = &workspace_manifest.manifest_path;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&path)
        .context(format!(
            "Can't open/create file for writing: {}",
            path.display()
        ))?;
    file.write_all(serde_json::to_string_pretty(&workspace_manifest)?.as_bytes())?;
    Ok(())
}

fn read_manifest(path: &std::path::PathBuf) -> Result<Manifest> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    Ok(serde_yaml::from_reader(reader)?)
}

/// Returns optional path to existing review workspace directory.
pub fn get_existing(
    package_name: &str,
    package_version: &str,
    registry_host_name: &str,
) -> Result<Option<Manifest>> {
    let package_unique_directory =
        get_unique_package_directory(&package_name, &package_version, &registry_host_name)?;
    let manifest_path = get_manifest_path(&package_unique_directory);
    if manifest_path.is_file() {
        Ok(Some(read_manifest(&manifest_path)?))
    } else {
        Ok(None)
    }
}

/// Extract and return archive file extension from archive URL.
fn get_archive_file_extension(artifact_url: &url::Url) -> Result<String> {
    let path = std::path::Path::new(artifact_url.path());
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
            artifact_url
        ))?
        .to_str()
        .ok_or(format_err!(
            "Failed to parse file extension unicode characters."
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

    for (_language_type, language) in &languages {
        for report in &language.reports {
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
        assert!(result == expected);
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

pub fn remove(workspace_manifest: &Manifest) -> Result<()> {
    log::debug!(
        "Removing workspace directory: {}",
        workspace_manifest.workspace_path.display()
    );
    std::fs::remove_dir_all(&workspace_manifest.workspace_path)?;

    if workspace_manifest.manifest_path.is_file() {
        log::debug!(
            "Removing workspace manifest file: {}",
            workspace_manifest.manifest_path.display()
        );
        std::fs::remove_file(&workspace_manifest.manifest_path)?;
    }

    let paths = common::fs::DataPaths::new()?;
    common::fs::remove_empty_directories(
        &workspace_manifest.workspace_path,
        &paths.ongoing_reviews_directory,
    )?;
    Ok(())
}
