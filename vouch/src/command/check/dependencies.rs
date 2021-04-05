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
    let local_dependencies =
        extension::identify_local_dependencies(&extensions, &working_directory)?;
    for (i, (extension, dependencies)) in extensions
        .iter()
        .zip(local_dependencies.into_iter())
        .enumerate()
    {
        log::info!(
            "Inspecting dependencies supported by extension: {}",
            extension.name()
        );
        let dependencies = match dependencies {
            Ok(dependencies) => dependencies,
            Err(error) => {
                log::error!("Extension error: {}", error);
                continue;
            }
        };

        let dependency_reports: Result<Vec<report::DependencyReport>> = dependencies
            .into_iter()
            .map(|dependency| -> Result<report::DependencyReport> {
                Ok(report::get_dependency_report(&dependency, &tx)?)
            })
            .collect();
        let dependency_reports = dependency_reports?;

        log::info!("Number of dependencies found: {}", dependency_reports.len());
        if dependency_reports.is_empty() {
            log::debug!(
                "Extension {} did not identify any dependencies in the \
            current working directory or parent directories.",
                extension.name()
            );
            continue;
        } else {
            definition_file_found = true;
        }

        let table = table::get(&dependency_reports)?;
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
