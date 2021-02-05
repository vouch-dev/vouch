use super::super::common;
use anyhow::Result;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommandResult {
    host_name: String,
}

pub fn run_command<T: common::Extension + std::fmt::Debug>(extension: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&CommandResult {
            host_name: extension.host_name(),
        })?
    );
    Ok(())
}
