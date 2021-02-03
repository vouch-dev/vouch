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
        Command::Follow(args) => {
            log::info!("Running command: follow");
            setup::is_complete()?;
            peer::add(&args)?;
        }
        Command::Unfollow(args) => {
            log::info!("Running command: unfollow");
            setup::is_complete()?;
            peer::remove(&args)?;
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
    /// Initial global setup.
    ///
    /// Initialise a local clone of a user's 'reviews' Git repository. Setup configuration settings.
    #[structopt(name = "setup")]
    Setup(setup::Arguments),

    /// Follow a reviewer.
    #[structopt(name = "follow")]
    Follow(peer::AddArguments),

    /// Unfollow a reviewer.
    #[structopt(name = "unfollow")]
    Unfollow(peer::RemoveArguments),

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
