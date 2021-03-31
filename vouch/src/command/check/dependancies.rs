use anyhow::Result;

use crate::common;
use crate::common::StoreTransaction;
use crate::extension;

use super::report;
use super::table;

pub fn report(
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<()> {
    let extensions = extension::get_enabled_extensions(&extension_names, &config)?;
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
            log::debug!(
                "Extension {} did not identify any dependancies in the \
            current working directory or parent directories.",
                extension.name()
            );
            continue;
        }

        let table = table::get(&dependancy_reports)?;
        println!("Extension: {}", extension.name());
        table.printstd();
    }
    Ok(())
}
