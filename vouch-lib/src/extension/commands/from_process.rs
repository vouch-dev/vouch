use super::super::common;
use anyhow::Result;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommandResult {
    host_name: String,
    root_url: String,
    package_url_template: String,
    package_version_url_template: String,
}

pub fn run_command<T: common::Extension + std::fmt::Debug>(extension: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&CommandResult {
            host_name: extension.host_name(),
            root_url: extension.root_url().to_string(),
            package_url_template: extension.package_url_template(),
            package_version_url_template: extension.package_version_url_template(),
        })?
    );
    Ok(())
}
