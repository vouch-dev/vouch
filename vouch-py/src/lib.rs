use anyhow::{format_err, Context, Result};
use std::{collections::HashSet, io::Read};
use strum::IntoEnumIterator;

mod pipfile;

#[derive(Clone, Debug)]
pub struct PyExtension {
    name_: String,
    host_name_: String,
    root_url_: url::Url,
    package_url_template_: String,
    package_version_url_template_: String,
}

impl vouch_lib::extension::Extension for PyExtension {
    fn new() -> Self {
        Self {
            name_: "py".to_string(),
            host_name_: "pypi.org".to_string(),
            root_url_: url::Url::parse("https://pypi.org/pypi").unwrap(),
            package_url_template_: "https://pypi.org/pypi/{{package_name}}/".to_string(),
            package_version_url_template_:
                "https://pypi.org/pypi/{{package_name}}/{{package_version}}/".to_string(),
        }
    }

    fn from_process(
        _process_path: &std::path::PathBuf,
        _extension_config_path: &std::path::PathBuf,
    ) -> Result<Self> {
        unimplemented!();
    }

    fn name(&self) -> String {
        self.name_.clone()
    }

    fn host_name(&self) -> String {
        self.host_name_.clone()
    }

    fn root_url(&self) -> url::Url {
        self.root_url_.clone()
    }

    fn package_url_template(&self) -> String {
        self.package_url_template_.clone()
    }

    fn package_version_url_template(&self) -> String {
        self.package_version_url_template_.clone()
    }

    fn identify_local_dependancies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<vouch_lib::extension::LocalDependancy>> {
        // Identify all dependancy definition files.
        let dependancy_files = match identify_dependancy_files(&working_directory) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        // Read all dependancies definitions files.
        let mut all_dependancies = HashSet::new();
        for dependancy_file in dependancy_files {
            // TODO: Handle all definition files.
            let dependancies: HashSet<vouch_lib::extension::LocalDependancy> =
                match dependancy_file.r#type {
                    DependancyFileType::PipfileLock => {
                        pipfile::get_dependancies(&dependancy_file.path)?
                    }
                    _ => HashSet::new(),
                };
            for dependancy in dependancies {
                all_dependancies.insert(dependancy);
            }
        }

        Ok(all_dependancies.into_iter().collect())
    }

    fn remote_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
        working_directory: &std::path::PathBuf,
    ) -> Result<vouch_lib::extension::RemotePackageMetadata> {
        let dependancy_files = identify_dependancy_files(&working_directory);
        if dependancy_files.is_none() {
            return Ok(vouch_lib::extension::RemotePackageMetadata {
                found_local_use: false,
                registry_package_url: None,
                registry_package_version_url: None,
                source_code_url: None,
                source_code_sha256: None,
            });
        }

        // Found local package dependancy file(s). Query remote package registry for given package.
        let found_local_use = dependancy_files.is_some();
        let registry_package_url = get_package_url(&self, &package_name)?;
        let registry_package_version_url =
            get_package_version_url(&self, &package_name, &package_version)?;

        let registry_package_url = match &registry_package_url {
            Some(v) => v,
            None => {
                return Ok(vouch_lib::extension::RemotePackageMetadata {
                    found_local_use,
                    registry_package_url: registry_package_url.map(|x| x.to_string()),
                    registry_package_version_url: registry_package_version_url
                        .map(|x| x.to_string()),
                    source_code_url: None,
                    source_code_sha256: None,
                });
            }
        };

        let entry_json = get_registry_entry_json(&registry_package_url)?;
        let source_code_url = get_source_code_url(&entry_json, &package_version)?;
        let source_code_sha256 = get_source_code_sha256(&entry_json, &package_version)?;

        Ok(vouch_lib::extension::RemotePackageMetadata {
            found_local_use,
            registry_package_url: Some(registry_package_url.to_string()),
            registry_package_version_url: registry_package_version_url.map(|x| x.to_string()),
            source_code_url: Some(source_code_url.to_string()),
            source_code_sha256: Some(source_code_sha256),
        })
    }
}

fn get_package_url(extension: &PyExtension, package_name: &str) -> Result<Option<url::Url>> {
    // Example return value: https://pypi.org/pypi/numpy/
    let handlebars_registry = handlebars::Handlebars::new();
    let url = handlebars_registry.render_template(
        &extension.package_url_template_,
        &maplit::btreemap! {
            "package_name" => package_name,
        },
    )?;
    Ok(Some(url::Url::parse(url.as_str())?))
}

fn get_package_version_url(
    extension: &PyExtension,
    package_name: &str,
    package_version: &str,
) -> Result<Option<url::Url>> {
    // Example return value: https://pypi.org/pypi/numpy/1.18.5/
    let handlebars_registry = handlebars::Handlebars::new();
    let registry_package_version_url = handlebars_registry.render_template(
        &extension.package_version_url_template_,
        &maplit::btreemap! {
            "package_name" => package_name,
            "package_version" => package_version,
        },
    )?;
    Ok(Some(url::Url::parse(
        registry_package_version_url.as_str(),
    )?))
}

fn get_registry_entry_json(registry_package_url: &url::Url) -> Result<serde_json::Value> {
    let json_url = registry_package_url.join("json")?;
    let mut result = reqwest::blocking::get(&json_url.to_string())?;
    let mut body = String::new();
    result.read_to_string(&mut body)?;

    Ok(serde_json::from_str(&body).context(format!("JSON was not well-formatted:\n{}", body))?)
}

fn get_source_code_url(
    registry_entry_json: &serde_json::Value,
    package_version: &str,
) -> Result<url::Url> {
    let releases = registry_entry_json["releases"][package_version]
        .as_array()
        .ok_or(format_err!("Failed to parse releases array."))?;
    for release in releases {
        let python_version = release["python_version"]
            .as_str()
            .ok_or(format_err!("Failed to parse package version."))?;
        if python_version == "source" {
            return Ok(url::Url::parse(release["url"].as_str().ok_or(
                format_err!("Failed to parse package source code URL."),
            )?)?);
        }
    }
    Err(format_err!("Failed to identify package source code URL."))
}

fn get_source_code_sha256(
    registry_entry_json: &serde_json::Value,
    package_version: &str,
) -> Result<String> {
    let releases = registry_entry_json["releases"][package_version]
        .as_array()
        .ok_or(format_err!("Failed to parse releases array."))?;
    for release in releases {
        let python_version = release["python_version"]
            .as_str()
            .ok_or(format_err!("Failed to parse python version."))?;
        if python_version == "source" {
            return Ok(release["digests"]["sha256"]
                .as_str()
                .ok_or(format_err!(
                    "Failed to parse package source code SHA256 hash."
                ))?
                .to_string());
        }
    }
    Err(format_err!(
        "Failed to identify package source code SHA256 hash."
    ))
}

/// Package dependancy file types.
#[derive(Debug, Copy, Clone, strum_macros::EnumIter)]
enum DependancyFileType {
    SetupPy,
    RequirementsTxt,
    PipfileLock,
}

impl DependancyFileType {
    /// Return file name associated with dependancy type.
    pub fn file_name(&self) -> std::path::PathBuf {
        match self {
            Self::SetupPy => std::path::PathBuf::from("setup.py"),
            Self::RequirementsTxt => std::path::PathBuf::from("requirements.txt"),
            Self::PipfileLock => std::path::PathBuf::from("Pipfile.lock"),
        }
    }
}

/// Package dependancy file type and file path.
#[derive(Debug, Clone)]
struct DependancyFile {
    r#type: DependancyFileType,
    path: std::path::PathBuf,
}

/// Returns a vector of identified package dependancy definition files.
///
/// Walks up the directory tree directory tree until the first positive result is found.
fn identify_dependancy_files(
    working_directory: &std::path::PathBuf,
) -> Option<Vec<DependancyFile>> {
    assert!(working_directory.is_absolute());
    let mut working_directory = working_directory.clone();

    loop {
        // If at least one target is found, assume package is present.
        let mut found_dependancy_file = false;

        let mut dependancy_files: Vec<DependancyFile> = Vec::new();
        for dependancy_file_type in DependancyFileType::iter() {
            let target_absolute_path = working_directory.join(dependancy_file_type.file_name());
            if target_absolute_path.is_file() {
                found_dependancy_file = true;
                dependancy_files.push(DependancyFile {
                    r#type: dependancy_file_type,
                    path: target_absolute_path,
                })
            }
        }
        if found_dependancy_file {
            return Some(dependancy_files);
        }

        // No need to move further up the directory tree after this loop.
        if working_directory == std::path::PathBuf::from("/") {
            break;
        }

        // Move further up the directory tree.
        working_directory.pop();
    }
    None
}
