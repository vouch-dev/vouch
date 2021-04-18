use anyhow::{format_err, Result};

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
