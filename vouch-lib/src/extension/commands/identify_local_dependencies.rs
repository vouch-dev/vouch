use super::super::common;
use anyhow::Result;
use structopt::{self, StructOpt};

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {
    /// Working directory.
    #[structopt(name = "working-directory")]
    pub working_directory: String,
}

pub fn run_command<T: common::Extension + std::fmt::Debug>(
    args: &Arguments,
    extension: &T,
) -> Result<()> {
    let working_directory = std::path::PathBuf::from(&args.working_directory);
    let local_dependencies = extension.identify_local_dependencies(&working_directory)?;
    println!("{}", serde_json::to_string(&local_dependencies)?);
    Ok(())
}
