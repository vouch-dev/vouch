use anyhow::{format_err, Context, Result};

use super::common;

#[derive(Debug, Clone)]
pub struct ProcessExtension {
    process_path: std::path::PathBuf,
    name_: String,
    host_name_: String,
}

impl common::Extension for ProcessExtension {
    fn new() -> Self
    where
        Self: Sized,
    {
        unimplemented!("Initialise this type with ProcessExtension::from_process.");
    }

    fn from_process(
        process_path: &std::path::PathBuf,
        extension_config_path: &std::path::PathBuf,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct StaticData {
            name: String,
            host_name: String,
        }

        let static_data: StaticData = if extension_config_path.is_file() {
            let file = std::fs::File::open(&extension_config_path)?;
            let reader = std::io::BufReader::new(file);
            serde_yaml::from_reader(reader)?
        } else {
            let static_data: Box<StaticData> = run_process(&process_path, &vec!["from-process"])?;
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
            process_path: process_path.clone(),
            name_: static_data.name,
            host_name_: static_data.host_name,
        })
    }

    fn name(&self) -> String {
        self.name_.clone()
    }

    fn host_name(&self) -> String {
        self.host_name_.clone()
    }

    /// Returns a list of local package dependancies which might also be hosted on the registry.
    fn identify_local_dependancies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<common::LocalDependancy>> {
        let working_directory = working_directory.to_str().ok_or(format_err!(
            "Failed to parse path into string: {}",
            working_directory.display()
        ))?;
        let args = vec!["identify-local-dependancies", working_directory];
        let output: Box<Vec<common::LocalDependancy>> = run_process(&self.process_path, &args)?;
        Ok(*output)
    }

    /// Given a package name and version, queries the remote registry for package metadata.
    fn remote_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
        working_directory: &std::path::PathBuf,
    ) -> Result<common::RemotePackageMetadata> {
        let working_directory = working_directory.to_str().ok_or(format_err!(
            "Failed to parse path into string: {}",
            working_directory.display()
        ))?;
        let args = vec![
            "remote-package-metadata",
            package_name,
            package_version,
            working_directory,
        ];
        let output: Box<common::RemotePackageMetadata> = run_process(&self.process_path, &args)?;
        Ok(*output)
    }
}

fn run_process<'a, T: ?Sized>(process_path: &std::path::PathBuf, args: &Vec<&str>) -> Result<Box<T>>
where
    for<'de> T: serde::Deserialize<'de> + 'a,
{
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
