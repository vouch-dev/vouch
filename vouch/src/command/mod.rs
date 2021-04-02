use anyhow::Result;
use structopt::{self, StructOpt};

mod check;
mod config;
mod peer;
mod review;
mod setup;
mod sync;

pub fn run_command(command: Command) -> Result<()> {
    match command {
        Command::Setup(args) => {
            log::info!("Running command: setup");
            setup::run_command(&args)?;
        }
        Command::Peer(subcommand) => {
            log::info!("Running command: peer");
            setup::is_complete()?;
            peer::run_subcommand(&subcommand)?;
        }
        Command::Review(args) => {
            log::info!("Running command: review");
            setup::is_complete()?;
            review::run_command(&args)?;
        }
        Command::Check(args) => {
            log::info!("Running command: check");
            setup::is_complete()?;
            check::run_command(&args)?;
        }
        Command::Sync(args) => {
            log::info!("Running command: sync");
            setup::is_complete()?;
            sync::run_command(&args)?;
        }
        Command::Config(args) => {
            log::info!("Running command: config");
            setup::is_complete()?;
            config::run_command(&args)?;
        }
    }
    Ok(())
}

#[derive(Debug, StructOpt, Clone)]
pub enum Command {
    /// Initial user setup.
    ///
    /// Initialise a local clone of a user's 'reviews' Git repository. Setup configuration settings.
    #[structopt(name = "setup")]
    Setup(setup::Arguments),

    /// Manage peers.
    #[structopt(name = "peer")]
    Peer(peer::Subcommands),

    /// Review a package.
    #[structopt(name = "review")]
    Review(review::Arguments),

    /// Check dependancies against reviews.
    #[structopt(name = "check")]
    Check(check::Arguments),

    /// Get updates from peers. Upload local changes.
    #[structopt(name = "sync")]
    Sync(sync::Arguments),

    /// Configure settings.
    #[structopt(name = "config")]
    Config(config::Arguments),
}

#[derive(Debug, StructOpt, Clone)]
#[structopt(about = "Package Reviews")]
#[structopt(global_setting = structopt::clap::AppSettings::ColoredHelp)]
#[structopt(global_setting = structopt::clap::AppSettings::DeriveDisplayOrder)]
pub struct Opts {
    #[structopt(subcommand)]
    pub command: Command,
}
