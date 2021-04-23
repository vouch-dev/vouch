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
    let extensions = extension::manage::get_enabled(&extension_names, &config)?;
    let working_directory = std::env::current_dir()?;
    log::debug!("Current working directory: {}", working_directory.display());

    let mut dependencies_found = false;
    let all_dependencies_specs =
        extension::identify_local_dependencies(&extensions, &working_directory)?;
    for (extension, extension_dependencies_specs) in
        extensions.iter().zip(all_dependencies_specs.into_iter())
    {
        log::info!(
            "Inspecting dependencies supported by extension: {}",
            extension.name()
        );

        let extension_dependencies_specs = match extension_dependencies_specs {
            Ok(d) => d,
            Err(error) => {
                log::error!("Extension error: {}", error);
                continue;
            }
        };
        for dependencies_spec in extension_dependencies_specs {
            dependencies_found |= !dependencies_spec.dependencies.is_empty();
            report_dependencies_spec(&dependencies_spec, &extension, &tx)?;
        }
    }

    if !dependencies_found {
        println!(
            "No dependency specification files found in \
            working directory or parent directories."
        )
    }
    Ok(())
}

fn report_dependencies_spec(
    dependencies_spec: &vouch_lib::extension::DependenciesSpec,
    extension: &Box<dyn vouch_lib::extension::Extension>,
    tx: &StoreTransaction,
) -> Result<()> {
    log::info!(
        "Generating report for dependencies specification file: {}",
        dependencies_spec.path.display()
    );
    let dependencies = &dependencies_spec.dependencies;

    let dependency_reports: Result<Vec<report::DependencyReport>> = dependencies
        .into_iter()
        .map(|dependency| -> Result<report::DependencyReport> {
            Ok(report::get_dependency_report(
                &dependency,
                &dependencies_spec.registry_host_name,
                &tx,
            )?)
        })
        .collect();
    let dependency_reports = dependency_reports?;

    log::info!("Number of dependencies found: {}", dependency_reports.len());
    if dependency_reports.is_empty() {
        return Ok(());
    }

    let table = table::get(&dependency_reports)?;
    println!(
        "\n\nExtension: {name}\n{path}",
        name = extension.name(),
        path = dependencies_spec.path.display(),
    );
    table.printstd();
    Ok(())
}
