use anyhow::{format_err, Context, Result};
use std::io::Read;
use strum::IntoEnumIterator;

mod pipfile;

#[derive(Clone, Debug)]
pub struct PyExtension {
    name_: String,
    registry_host_names_: Vec<String>,
    root_url_: url::Url,
    package_url_template_: String,
    registry_human_url_template_: String,
}

impl vouch_lib::extension::FromLib for PyExtension {
    fn new() -> Self {
        Self {
            name_: "py".to_string(),
            registry_host_names_: vec!["pypi.org".to_owned()],
            root_url_: url::Url::parse("https://pypi.org/pypi").unwrap(),
            package_url_template_: "https://pypi.org/pypi/{{package_name}}/".to_string(),
            registry_human_url_template_:
                "https://pypi.org/pypi/{{package_name}}/{{package_version}}/".to_string(),
        }
    }
}

impl vouch_lib::extension::Extension for PyExtension {
    fn name(&self) -> String {
        self.name_.clone()
    }

    fn registries(&self) -> Vec<String> {
        self.registry_host_names_.clone()
    }

    fn identify_local_dependencies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<vouch_lib::extension::DependenciesSpec>> {
        // Identify all dependency definition files.
        let dependency_files = match identify_dependency_files(&working_directory) {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        // Read all dependencies definitions files.
        let mut all_dependency_specs = Vec::new();
        for dependency_file in dependency_files {
            // TODO: Add support for parsing all definition file types.
            let (dependencies, registry_host_name) = match dependency_file.r#type {
                DependencyFileType::PipfileLock => (
                    pipfile::get_dependencies(&dependency_file.path)?,
                    pipfile::get_registry_host_name(),
                ),
            };
            all_dependency_specs.push(vouch_lib::extension::DependenciesSpec {
                path: dependency_file.path,
                registry_host_name: registry_host_name,
                dependencies: dependencies.into_iter().collect(),
            });
        }

        Ok(all_dependency_specs)
    }

    fn remote_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<vouch_lib::extension::RemotePackageMetadata> {
        // Currently, only one registry is supported. Therefore simply select first.
        let registry_host_name = self
            .registries()
            .first()
            .ok_or(format_err!(
                "Code error: vector of registry host names is empty."
            ))?
            .clone();

        let entry_json = get_registry_entry_json(&package_name)?;
        let archive_url = get_archive_url(&entry_json, &package_version)?;
        let human_url = get_registry_human_url(&self, &package_name, &package_version)?;

        Ok(vouch_lib::extension::RemotePackageMetadata {
            registry_host_name: registry_host_name,
            human_url: human_url.to_string(),
            archive_url: archive_url.to_string(),
        })
    }
}

fn get_registry_human_url(
    extension: &PyExtension,
    package_name: &str,
    package_version: &str,
) -> Result<url::Url> {
    // Example return value: https://pypi.org/pypi/numpy/1.18.5/
    let handlebars_registry = handlebars::Handlebars::new();
    let human_url = handlebars_registry.render_template(
        &extension.registry_human_url_template_,
        &maplit::btreemap! {
            "package_name" => package_name,
            "package_version" => package_version,
        },
    )?;
    Ok(url::Url::parse(human_url.as_str())?)
}

fn get_registry_entry_json(package_name: &str) -> Result<serde_json::Value> {
    let handlebars_registry = handlebars::Handlebars::new();
    let url = handlebars_registry.render_template(
        "https://pypi.org/pypi/{{package_name}}/json",
        &maplit::btreemap! {
            "package_name" => package_name,
        },
    )?;
    let mut result = reqwest::blocking::get(&url.to_string())?;
    let mut body = String::new();
    result.read_to_string(&mut body)?;

    Ok(serde_json::from_str(&body).context(format!("JSON was not well-formatted:\n{}", body))?)
}

fn get_archive_url(
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
            return Ok(url::Url::parse(
                release["url"]
                    .as_str()
                    .ok_or(format_err!("Failed to parse package archive URL."))?,
            )?);
        }
    }
    Err(format_err!("Failed to identify package archive URL."))
}

/// Package dependency file types.
#[derive(Debug, Copy, Clone, strum_macros::EnumIter)]
enum DependencyFileType {
    PipfileLock,
}

impl DependencyFileType {
    /// Return file name associated with dependency type.
    pub fn file_name(&self) -> std::path::PathBuf {
        match self {
            Self::PipfileLock => std::path::PathBuf::from("Pipfile.lock"),
        }
    }
}

/// Package dependency file type and file path.
#[derive(Debug, Clone)]
struct DependencyFile {
    r#type: DependencyFileType,
    path: std::path::PathBuf,
}

/// Returns a vector of identified package dependency definition files.
///
/// Walks up the directory tree directory tree until the first positive result is found.
fn identify_dependency_files(
    working_directory: &std::path::PathBuf,
) -> Option<Vec<DependencyFile>> {
    assert!(working_directory.is_absolute());
    let mut working_directory = working_directory.clone();

    loop {
        // If at least one target is found, assume package is present.
        let mut found_dependency_file = false;

        let mut dependency_files: Vec<DependencyFile> = Vec::new();
        for dependency_file_type in DependencyFileType::iter() {
            let target_absolute_path = working_directory.join(dependency_file_type.file_name());
            if target_absolute_path.is_file() {
                found_dependency_file = true;
                dependency_files.push(DependencyFile {
                    r#type: dependency_file_type,
                    path: target_absolute_path,
                })
            }
        }
        if found_dependency_file {
            return Some(dependency_files);
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
