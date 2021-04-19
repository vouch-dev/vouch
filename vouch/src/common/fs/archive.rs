use anyhow::{format_err, Result};
use std::io::Write;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArchiveType {
    Zip,
    TarGz,
    Tgz,
    Unknown,
}

impl std::convert::TryFrom<&std::path::PathBuf> for ArchiveType {
    type Error = anyhow::Error;

    fn try_from(path: &std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(match get_file_extension(&path)?.as_str() {
            "zip" => Self::Zip,
            "tar.gz" => Self::TarGz,
            "tgz" => Self::Tgz,
            _ => Self::Unknown,
        })
    }
}

impl ArchiveType {
    pub fn to_string(&self) -> Result<String> {
        Ok(match self {
            ArchiveType::Zip => "zip",
            ArchiveType::TarGz => "tar.gz",
            ArchiveType::Tgz => "tgz",
            ArchiveType::Unknown => {
                return Err(format_err!(
                    "Failed to convert unknown archive type into string."
                ))
            }
        }
        .to_string())
    }
}

/// Extract and return archive file extension from given path.
fn get_file_extension(path: &std::path::PathBuf) -> Result<String> {
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
            "Failed to parse file extension from path: {}",
            path.display()
        ))?
        .to_str()
        .ok_or(format_err!(
            "Failed to parse file extension unicode characters."
        ))?
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_extension_extracted_for_tar_gz() -> Result<()> {
        let result = get_file_extension(&std::path::PathBuf::from("/d3/d3-4.10.0.tar.gz"))?;
        let expected = "tar.gz".to_string();
        assert!(result == expected);
        Ok(())
    }
}

pub fn extract_zip(
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
pub fn extract_tar_gz(
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

pub fn download(target_url: &url::Url, destination_path: &std::path::PathBuf) -> Result<()> {
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
