use anyhow::Result;
use structopt::{self, StructOpt};

use crate::common;
use crate::extension;
use crate::store;

mod dependancies;
mod report;
mod specific;
mod table;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {
    /// Package name.
    #[structopt(name = "name")]
    pub package_name: Option<String>,

    /// Package version.
    #[structopt(name = "version", requires("name"))]
    pub package_version: Option<String>,

    /// Specify an extension for handling the package or dependancies.
    /// Example values: py, js, rs
    #[structopt(long = "extension", short = "e", name = "name")]
    pub extension_names: Option<Vec<String>>,
}

pub fn run_command(args: &Arguments) -> Result<()> {
    let mut config = common::config::Config::load()?;
    extension::update_config(&mut config)?;
    let config = config;
    let extension_names = extension::handle_extension_names_arg(&args.extension_names, &config)?;

    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    match &args.package_name {
        Some(package_name) => {
            specific::report(
                &package_name,
                &args.package_version,
                &extension_names,
                &config,
                &tx,
            )?;
        }
        None => {
            dependancies::report(&config, &tx)?;
        }
    }
    Ok(())
}
