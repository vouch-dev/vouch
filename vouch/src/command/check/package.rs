use anyhow::Result;

use crate::common;
use crate::common::StoreTransaction;
use crate::extension;
use crate::review;

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
    let all_dependencies_specs = extension::identify_package_dependencies(
        &package_name,
        &package_version,
        &extensions,
        &extension_args,
    )?;
    for (extension, extension_dependencies_specs) in
        extensions.iter().zip(all_dependencies_specs.into_iter())
    {
        log::debug!(
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

        for (index, dependencies_spec) in extension_dependencies_specs.iter().enumerate() {
            dependencies_found |= !dependencies_spec.dependencies.is_empty();
            report_dependencies(&package_name, &dependencies_spec, &tx)?;
            let is_last = index == extension_dependencies_specs.len() - 1;
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

// /// Prints a report for a specific package.
// pub fn report(
//     package_name: &str,
//     package_version: &Option<String>,
//     extension_names: &std::collections::BTreeSet<String>,
//     config: &common::config::Config,
//     tx: &StoreTransaction,
// ) -> Result<()> {
//     // TODO: Handle multiple registries.
//     let reviews = get_package_reviews(
//         package_name,
//         package_version,
//         &extension_names,
//         &config,
//         &tx,
//     )?;
//     if reviews.is_empty() {
//         if let Some(package_version) = package_version {
//             println!(
//                 "No reviews found for package {} at version {}.",
//                 package_name, package_version
//             );
//         } else {
//             println!("No reviews found for package {}.", package_name);
//         }
//         let disabled_extension_names = extension::manage::get_disabled_names(&config)?;
//         if !disabled_extension_names.is_empty() {
//             println!(
//                 "Consider enabling some of these extensions: {}",
//                 disabled_extension_names.join(", ")
//             )
//         }
//     }

//     let mut reviews_by_version = std::collections::BTreeMap::<_, Vec<&review::Review>>::new();
//     for review in &reviews {
//         match reviews_by_version.get_mut(&review.package.version) {
//             Some(review_group) => review_group.push(&review),
//             None => {
//                 reviews_by_version.insert(review.package.version.clone(), vec![&review]);
//             }
//         }
//     }

//     for package_version in reviews_by_version.keys().rev() {
//         println!(
//             "{name} {version}\n",
//             name = package_name,
//             version = package_version
//         );

//         let mut reviews = match reviews_by_version.get(package_version) {
//             Some(reviews) => reviews.clone(),
//             None => Vec::<_>::new(),
//         };
//         reviews.sort();

//         let mut reports = Vec::<_>::new();
//         for review in reviews {
//             let report = get_review_report(&review)?;
//             reports.push(report);
//         }
//         println!("{}", reports.join("\n\n"));
//     }
//     Ok(())
// }

// fn get_review_report(review: &review::Review) -> Result<String> {
//     let peer_url = if !review.peer.is_root() {
//         format!("({})", review.peer.git_url)
//     } else {
//         "".to_string()
//     };

//     Ok(format!(
//         "\
// Peer: {peer_alias} {peer_url}\n\
// {comments_report}
// \n\n",
//         peer_alias = review.peer.alias,
//         peer_url = peer_url,
//         comments_report = serde_yaml::to_string(&review.comments)?,
//     ))
// }

// fn get_package_reviews(
//     package_name: &str,
//     package_version: &Option<String>,
//     extension_names: &std::collections::BTreeSet<String>,
//     config: &common::config::Config,
//     tx: &StoreTransaction,
// ) -> Result<std::collections::BTreeSet<review::Review>> {
//     let registry_host_names =
//         extension::manage::get_enabled_registry_host_names(&extension_names, &config)?;
//     let registry_host_names = registry_host_names
//         .iter()
//         .map(|host_name| host_name.as_str())
//         .collect();

//     let reviews = review::index::get(
//         &review::index::Fields {
//             package_name: Some(package_name),
//             package_version: package_version.as_deref(),
//             registry_host_names: Some(registry_host_names),
//             ..Default::default()
//         },
//         &tx,
//     )?
//     .into_iter()
//     .collect::<std::collections::BTreeSet<_>>();
//     Ok(reviews)
// }
