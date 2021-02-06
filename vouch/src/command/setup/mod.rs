use anyhow::{format_err, Result};
use std::convert::TryFrom;
use structopt::{self, StructOpt};

use crate::store;
mod fs;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {
    /// Git repository URL.
    #[structopt(name = "git-url", parse(try_from_str = crate::common::GitUrl::try_from))]
    pub git_url: Option<crate::common::GitUrl>,

    /// Force setup cleanly. Removes existing local setup data.
    #[structopt(long = "force", short = "f")]
    pub force: bool,
}

pub fn run_command(args: &Arguments) -> Result<()> {
    fs::setup(&args.git_url, args.force)?;

    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    store::index::setup(&tx)?;

    tx.commit("Setup Vouch.")?;
    Ok(())
}

/// Return Err if setup is not complete, otherwise Result.
pub fn is_complete() -> Result<()> {
    if !fs::is_complete()? {
        return Err(format_err!(
            "Setup command has not been executed. Try running: 'vouch setup --help'"
        ));
    }
    Ok(())
}
