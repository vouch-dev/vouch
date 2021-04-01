use super::super::common;
use anyhow::Result;

pub fn run_command<T: common::Extension + std::fmt::Debug>(extension: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&crate::extension::process::StaticData {
            name: extension.name(),
            registry_host_names: extension.registries()
        })?
    );
    Ok(())
}
