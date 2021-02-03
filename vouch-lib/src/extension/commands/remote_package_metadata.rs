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
    /// Package name.
    #[structopt(name = "package-name")]
    pub package_name: String,

    /// Package version.
    #[structopt(name = "package-version")]
    pub package_version: String,

    /// Working directory.
    #[structopt(name = "working-directory")]
    pub working_directory: String,
}

pub fn run_command<T: common::Extension + std::fmt::Debug>(
    args: &Arguments,
    extension: &mut T,
) -> Result<()> {
    let working_directory = std::path::PathBuf::from(&args.working_directory);
    let local_dependancies = extension.remote_package_metadata(
        &args.package_name,
        &args.package_version,
        &working_directory,
    )?;
    println!("{}", serde_json::to_string(&local_dependancies)?);
    Ok(())
}
