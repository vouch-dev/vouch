use anyhow::Result;

use crate::common;
use crate::common::StoreTransaction;
use crate::extension;

use super::report;
use super::table;

/// Prints a report for a specific package.
pub fn report(
    package_name: &str,
    package_version: &Option<&str>,
    extension_names: &std::collections::BTreeSet<String>,
    extension_args: &Vec<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<()> {
    let extensions = extension::manage::get_enabled(&extension_names, &config)?;

    let mut dependencies_found = false;
    let all_extensions_results = extension::identify_package_dependencies(
        &package_name,
        &package_version,
        &extensions,
        &extension_args,
    )?;

    let mut extensions_results = vec![];
    for (extension, extension_all_dependencies) in
        extensions.iter().zip(all_extensions_results.iter())
    {
        let all_dependencies = match extension_all_dependencies {
            Ok(d) => d,
            Err(error) => {
                log::error!(
                    "Extension {name} error: {error}",
                    name = extension.name(),
                    error = error
                );
                continue;
            }
        };
        extensions_results.push((extension, all_dependencies));
    }

    let all_dependencies = extensions_results
        .iter()
        .map(|(_ext, deps)| deps.clone())
        .flatten()
        .collect();
    let official_reviews = crate::review::official::get(&all_dependencies, &config.core.api_key)?;

    for (extension, extension_all_dependencies) in
        extensions.iter().zip(all_extensions_results.into_iter())
    {
        log::debug!(
            "Inspecting dependencies supported by extension: {}",
            extension.name()
        );

        let extension_all_package_dependencies = match extension_all_dependencies {
            Ok(d) => d,
            Err(error) => {
                log::error!("Extension error: {}", error);
                continue;
            }
        };

        for (index, package_dependencies) in extension_all_package_dependencies.iter().enumerate() {
            dependencies_found |= !package_dependencies.dependencies.is_empty();
            report_dependencies(&package_name, &package_dependencies, &tx)?;
            let is_last = index == extension_all_package_dependencies.len() - 1;
            if !is_last {
                println!("");
            }
        }
    }

    if !dependencies_found {
        println!("No dependencies found.")
    }
    Ok(())
}

fn report_dependencies(
    package_name: &str,
    package_dependencies: &vouch_lib::extension::PackageDependencies,
    tx: &StoreTransaction,
) -> Result<()> {
    log::info!("Generating report for package dependencies.");
    let dependencies = &package_dependencies.dependencies;

    let mut dependency_reports = vec![];
    let target_package_dependency_report = report::get_dependency_report(
        &vouch_lib::extension::Dependency {
            name: package_name.to_string(),
            version: package_dependencies.package_version.clone(),
        },
        &package_dependencies.registry_host_name,
        &tx,
    )?;
    dependency_reports.push(target_package_dependency_report);
    for dependency in dependencies {
        let dependency_report = report::get_dependency_report(
            &dependency,
            &package_dependencies.registry_host_name,
            &tx,
        )?;
        dependency_reports.push(dependency_report);
    }

    log::info!("Number of dependencies found: {}", dependency_reports.len());
    if dependency_reports.is_empty() {
        return Ok(());
    }

    println!(
        "Registry: {name}",
        name = package_dependencies.registry_host_name
    );

    let table = table::get(&dependency_reports, true)?;
    table.printstd();
    Ok(())
}
