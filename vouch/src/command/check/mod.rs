use anyhow::Result;
use structopt::{self, StructOpt};

use crate::common;
use crate::extension;
use crate::store;

mod fs;
mod package;
mod report;
mod table;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {
    /// Package name.
    #[structopt(name = "package-name")]
    pub package_name: Option<String>,

    /// Package version.
    #[structopt(name = "package-version", requires("package-name"))]
    pub package_version: Option<String>,

    /// Specify an extension for handling the package or dependencies.
    /// Example values: py, js, rs
    #[structopt(long = "extension", short = "e", name = "name")]
    pub extension_names: Option<Vec<String>>,
}

pub fn run_command(args: &Arguments, extension_args: &Vec<String>) -> Result<()> {
    let mut config = common::config::Config::load()?;
    extension::manage::update_config(&mut config)?;
    let config = config;
    let extension_names =
        extension::manage::handle_extension_names_arg(&args.extension_names, &config)?;

    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    match &args.package_name {
        Some(package_name) => {
            package::report(
                &package_name,
                &args.package_version.as_deref(),
                &extension_names,
                &extension_args,
                &config,
                &tx,
            )?;
        }
        None => {
            fs::report(&extension_names, &extension_args, &config, &tx)?;
        }
    }
    Ok(())
}
