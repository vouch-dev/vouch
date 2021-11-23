use anyhow::{format_err, Context, Result};

use super::common;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct StaticData {
    pub name: String,
    pub registry_host_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessExtension {
    process_path_: std::path::PathBuf,
    name_: String,
    registry_host_names_: Vec<String>,
}

impl common::FromProcess for ProcessExtension {
    fn from_process(
        process_path: &std::path::PathBuf,
        extension_config_path: &std::path::PathBuf,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let static_data: StaticData = if extension_config_path.is_file() {
            let file = std::fs::File::open(&extension_config_path)?;
            let reader = std::io::BufReader::new(file);
            serde_yaml::from_reader(reader)?
        } else {
            let static_data: Box<StaticData> = run_process(&process_path, &vec!["static-data"])?;
            let static_data = *static_data;

            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&extension_config_path)
                .context(format!(
                    "Can't open/create file for writing: {}",
                    extension_config_path.display()
                ))?;
            let writer = std::io::BufWriter::new(file);
            serde_yaml::to_writer(writer, &static_data)?;
            static_data
        };

        Ok(ProcessExtension {
            process_path_: process_path.clone(),
            name_: static_data.name,
            registry_host_names_: static_data.registry_host_names,
        })
    }
}

impl common::Extension for ProcessExtension {
    fn name(&self) -> String {
        self.name_.clone()
    }

    fn registries(&self) -> Vec<String> {
        self.registry_host_names_.clone()
    }

    /// Returns a list of local package dependencies specification files.
    fn identify_local_dependencies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<common::DependenciesSpec>> {
        let working_directory = working_directory.to_str().ok_or(format_err!(
            "Failed to parse path into string: {}",
            working_directory.display()
        ))?;
        let args = vec!["identify-local-dependencies", working_directory];
        let output: Box<Vec<common::DependenciesSpec>> = run_process(&self.process_path_, &args)?;
        Ok(*output)
    }

    /// Given a package name and version, queries the remote registry for package metadata.
    fn registries_package_metadata(
        &self,
        package_name: &str,
        package_version: &Option<&str>,
    ) -> Result<Vec<common::RegistryPackageMetadata>> {
        let mut args = vec!["registries-package-metadata", package_name];
        if let Some(package_version) = package_version {
            args.push(package_version.clone());
        }

        let output: Box<Vec<common::RegistryPackageMetadata>> =
            run_process(&self.process_path_, &args)?;
        Ok(*output)
    }
}

fn run_process<'a, T: ?Sized>(process_path: &std::path::PathBuf, args: &Vec<&str>) -> Result<Box<T>>
where
    for<'de> T: serde::Deserialize<'de> + 'a,
{
    log::debug!(
        "Executing extensions process call with arguments\n{:?}",
        args
    );
    let process = process_path.to_str().ok_or(format_err!(
        "Failed to parse string from process path: {}",
        process_path.display()
    ))?;
    let handle = std::process::Command::new(process)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&handle.stdout);
    let output = serde_json::from_str(&stdout)?;
    Ok(Box::new(output))
}
