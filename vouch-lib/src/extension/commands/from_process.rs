use super::super::common;
use anyhow::Result;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommandResult {}

pub fn run_command<T: common::Extension + std::fmt::Debug>(_extension: &T) -> Result<()> {
    println!("{}", serde_json::to_string(&CommandResult {})?);
    Ok(())
}
