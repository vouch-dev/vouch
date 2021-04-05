use super::common;
use anyhow::Result;
use structopt::{self, StructOpt};

mod identify_local_dependencies;
mod remote_package_metadata;
mod static_data;

#[derive(Debug, StructOpt, Clone)]
enum Command {
    /// Get extension static data.
    #[structopt(name = "static-data")]
    StaticData,

    /// Identify local dependencies.
    #[structopt(name = "identify-local-dependencies")]
    IdentifyLocalDependencies(identify_local_dependencies::Arguments),

    /// Get remote package metadata.
    #[structopt(name = "remote-package-metadata")]
    RemotePackageMetadata(remote_package_metadata::Arguments),
}

fn run_command<T: common::Extension + std::fmt::Debug>(
    command: Command,
    extension: &mut T,
) -> Result<()> {
    match command {
        Command::StaticData => {
            static_data::run_command(extension)?;
        }

        Command::IdentifyLocalDependencies(args) => {
            identify_local_dependencies::run_command(&args, extension)?;
        }

        Command::RemotePackageMetadata(args) => {
            remote_package_metadata::run_command(&args, extension)?;
        }
    }
    Ok(())
}

#[derive(Debug, StructOpt, Clone)]
#[structopt(about = "Package Reviews")]
#[structopt(global_setting = structopt::clap::AppSettings::ColoredHelp)]
#[structopt(global_setting = structopt::clap::AppSettings::DeriveDisplayOrder)]
struct Opts {
    #[structopt(subcommand)]
    pub command: Command,
}

pub fn run<T: common::Extension + std::fmt::Debug>(extension: &mut T) -> Result<()> {
    let commands = Opts::from_args();
    match run_command(commands.command, extension) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(-2)
        }
    };
    Ok(())
}
