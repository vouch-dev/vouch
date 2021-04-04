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

    let mut definition_file_found = false;
    let local_dependancies =
        extension::identify_local_dependancies(&extensions, &working_directory)?;
    for (i, (extension, dependancies)) in extensions
        .iter()
        .zip(local_dependancies.into_iter())
        .enumerate()
    {
        log::info!(
            "Inspecting dependancies supported by extension: {}",
            extension.name()
        );
        let dependancies = match dependancies {
            Ok(dependancies) => dependancies,
            Err(error) => {
                log::error!("Extension error: {}", error);
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

        log::info!("Number of dependancies found: {}", dependancy_reports.len());
        if dependancy_reports.is_empty() {
            log::debug!(
                "Extension {} did not identify any dependancies in the \
            current working directory or parent directories.",
                extension.name()
            );
            continue;
        } else {
            definition_file_found = true;
        }

        let table = table::get(&dependancy_reports)?;
        let inter_extension_padding = if i > 0 { "\n\n" } else { "" };
        println!(
            "{inter_extension_padding}Extension: {name}",
            inter_extension_padding = inter_extension_padding,
            name = extension.name()
        );
        table.printstd();
    }

    if !definition_file_found {
        println!("No definition files found in working directory or parent directories.")
    }
    Ok(())
}
