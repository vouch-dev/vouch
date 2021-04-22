use anyhow::{format_err, Result};
use std::convert::TryFrom;
use std::os::unix::fs::PermissionsExt;

use crate::common;
mod github;

pub fn add_from_url(
    url: &url::Url,
    extensions_bin_directory: &std::path::PathBuf,
) -> Result<String> {
    let archive_url = if is_supported_archive_url(&url)? {
        url.clone()
    } else {
        match get_archive_url(&url)? {
            Some(url) => url,
            None => {
                return Err(format_err!(
                    "Failed to obtain suitable release archive URL."
                ))
            }
        }
    };
    log::info!("Using archive URL: {}", archive_url);

    let archive_type =
        common::fs::archive::ArchiveType::try_from(&std::path::PathBuf::from(archive_url.path()))?;

    let tmp_dir = tempdir::TempDir::new("vouch_extension_add")?;
    let tmp_directory_path = tmp_dir.path().to_path_buf();
    log::info!(
        "Downloading extension archive to temporary directory: {}",
        tmp_directory_path.display()
    );
    let archive_path =
        tmp_directory_path.join(format!("archive.{}", archive_type.try_to_string()?));

    common::fs::archive::download(&archive_url, &archive_path)?;
    common::fs::archive::extract(&archive_path, &tmp_directory_path)?;

    let (bin_path, extension_name) = get_extension_bin_metadata(&tmp_directory_path)?.ok_or(
        format_err!("Failed to identify extension binary in archive."),
    )?;
    log::info!(
        "Identified binary for extension {}: {}",
        extension_name,
        bin_path.display()
    );
    let bin_file_name = bin_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(format_err!("Failed to derive extension binary file name."))?;

    let bin_destination_path = extensions_bin_directory.join(bin_file_name);
    log::info!("Copying binary to path: {}", bin_destination_path.display());
    std::fs::copy(&bin_path, &bin_destination_path)?;

    ensure_executable_permissions(&bin_destination_path)?;

    tmp_dir.close()?;
    Ok(extension_name)
}

#[cfg(target_family = "unix")]
fn ensure_executable_permissions(path: &std::path::PathBuf) -> Result<()> {
    log::debug!(
        "Setting executable permissions to 755 for file: {}",
        path.display()
    );
    let permissions = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&path, permissions)?;
    Ok(())
}

#[cfg(not(target_family = "unix"))]
fn ensure_executable_permissions(_path: &std::path::PathBuf) -> Result<()> {
    Ok(())
}

fn get_extension_bin_metadata(
    directory: &std::path::PathBuf,
) -> Result<Option<(std::path::PathBuf, String)>> {
    let regex_pattern = get_extension_bin_name_regex()?;
    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = get_extension_bin_name(&path, &regex_pattern)? {
            return Ok(Some((path, name)));
        }
    }
    Ok(None)
}

fn get_extension_bin_name_regex() -> Result<regex::Regex> {
    Ok(regex::Regex::new(r"vouch-(?P<name>[a-zA-Z0-9]*)(\.exe)?$")?)
}

fn get_extension_bin_name(
    path: &std::path::PathBuf,
    regex_pattern: &regex::Regex,
) -> Result<Option<String>> {
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        match regex_pattern.captures(file_name) {
            Some(captures) => Ok(Some(captures["name"].to_string())),
            None => Ok(None),
        }
    } else {
        Ok(None)
    }
}

fn is_supported_archive_url(url: &url::Url) -> Result<bool> {
    let path = std::path::PathBuf::from(url.path());
    Ok(common::fs::archive::ArchiveType::try_from(&path)?
        != common::fs::archive::ArchiveType::Unknown)
}

/// Returns a release archive URL.
fn get_archive_url(url: &url::Url) -> Result<Option<url::Url>> {
    Ok(if url.host_str() == Some("github.com") {
        github::get_archive_url(&url)?
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    mod get_extension_bin_name {
        use super::*;

        #[test]
        fn test_matching_file_name() -> Result<()> {
            let regex_pattern = get_extension_bin_name_regex()?;
            let bin_path =
                std::path::PathBuf::from("/tmp/vouch-extension_add/vouch-python/vouch-py");
            let result = get_extension_bin_name(&bin_path, &regex_pattern)?;
            let expected = Some("py".to_string());
            assert_eq!(result, expected);
            Ok(())
        }

        #[test]
        fn test_not_matching_file_name() -> Result<()> {
            let regex_pattern = get_extension_bin_name_regex()?;
            let bin_path =
                std::path::PathBuf::from("/tmp/vouch-extension_add/vouch-python/vouch-py.d");
            let result = get_extension_bin_name(&bin_path, &regex_pattern)?;
            let expected = None;
            assert_eq!(result, expected);
            Ok(())
        }
    }
}
