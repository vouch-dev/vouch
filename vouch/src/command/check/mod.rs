use anyhow::Result;
use structopt::{self, StructOpt};

use crate::review;
use crate::store;
use crate::{common, extension};

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
    #[structopt(name = "name")]
    pub package_name: Option<String>,

    /// Package version.
    #[structopt(name = "version", requires("package_name"))]
    pub package_version: Option<String>,
}

pub fn run_command(args: &Arguments) -> Result<()> {
    let config = common::config::Config::load()?;
    match &args.package_name {
        Some(package_name) => {
            single_dependancy_report(&package_name, &args.package_version)?;
        }
        None => {
            local_dependancies_table(&config)?;
        }
    }
    Ok(())
}

fn single_dependancy_report(package_name: &str, package_version: &Option<String>) -> Result<()> {
    // TODO: Handle multiple registries.
    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    let mut reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(package_name),
            package_version: package_version.as_deref(),
            ..Default::default()
        },
        &tx,
    )?;
    reviews.sort_by_cached_key(|review| {
        (
            review.package_security.clone(),
            review.review_confidence.clone(),
        )
    });

    let mut reviews_by_version = std::collections::BTreeMap::<_, Vec<&review::Review>>::new();
    for review in &reviews {
        match reviews_by_version.get_mut(&review.package.version) {
            Some(review_group) => review_group.push(&review),
            None => {
                reviews_by_version.insert(review.package.version.clone(), vec![&review]);
            }
        }
    }

    for package_version in reviews_by_version.keys().rev() {
        println!(
            "{name} {version}\n",
            name = package_name,
            version = package_version
        );

        for review in reviews_by_version.get(package_version).unwrap_or(&vec![]) {
            let peer_url = if !review.peer.is_root() {
                format!("({})", review.peer.git_url)
            } else {
                "".to_string()
            };
            println!(
                "\
                \t\tPeer:              {peer_alias} {peer_url}\n\
                \t\tPackage security:  {package_security}\n\
                \t\tReview confidence: {review_confidence}\n\
            \n\n",
                peer_alias = review.peer.alias,
                peer_url = peer_url,
                package_security = review.package_security,
                review_confidence = review.review_confidence,
            );
        }
    }
    Ok(())
}

fn local_dependancies_table(config: &common::config::Config) -> Result<()> {
    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    let extensions = extension::get_enabled_extensions(&config)?;
    let working_directory = std::env::current_dir()?;
    log::debug!("Current working directory: {}", working_directory.display());

    let local_dependancies =
        extension::identify_local_dependancies(&extensions, &working_directory)?;
    for (extension, dependancies) in extensions.iter().zip(local_dependancies.into_iter()) {
        log::info!(
            "Inspecting dependancies supported by extension: {}",
            extension.name()
        );
        let dependancies = match dependancies {
            Ok(v) => v,
            Err(e) => {
                log::error!("Extension error: {error}", error = e);
                continue;
            }
        };

        let dependancy_reports: Result<Vec<report::DependancyReport>> = dependancies
            .into_iter()
            .map(|dependancy| -> Result<report::DependancyReport> {
                Ok(report::get_dependancy_report(&dependancy, &tx)?)
            })
            .collect();
        let dependancy_reports = dependancy_reports?;

        log::info!(
            "Number of dependancies found: {count_dependancies}",
            count_dependancies = dependancy_reports.len(),
        );
        if dependancy_reports.is_empty() {
            log::debug!("Extension did not identify any dependancies in the current working directory or parent directories.");
            continue;
        }

        let table = table::get(&dependancy_reports)?;
        println!("Ecosystem: {}", extension.name());
        table.printstd();
    }
    Ok(())
}
