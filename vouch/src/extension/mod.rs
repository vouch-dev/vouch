use anyhow::{format_err, Result};
use crossbeam_utils;

mod common;
pub mod manage;
mod process;

/// Search package registries via extensions for package metadata from registries.
///
/// Returns a vector of results where each element describes metadata for a given registry. All elements
/// within the return vector correspond to the same ecosystem.
/// Raises errors for no results or results which span multiple ecosystems. Ok for single result.
pub fn search_registries<'a>(
    package_name: &str,
    package_version: &Option<&str>,
    extensions: &'a Vec<Box<dyn vouch_lib::extension::Extension>>,
) -> Result<Vec<vouch_lib::extension::RegistryPackageMetadata>> {
    type SearchResults = Result<Vec<Result<Vec<vouch_lib::extension::RegistryPackageMetadata>>>>;
    let search_results: SearchResults = crossbeam_utils::thread::scope(|s| {
        let threads: Vec<_> = extensions
            .iter()
            .map(|extension| {
                s.spawn(move |_| {
                    extension.registries_package_metadata(&package_name, &package_version)
                })
            })
            .collect();
        Ok(threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect())
    })
    .unwrap();

    let extensions_search_results = search_results
        .map(|search_result| search_result.into_iter().zip(extensions.iter()).collect())?;
    select_search_result(extensions_search_results)
}

/// Parses potentially multi-result search output. Handles no result or multiple result cases.
fn select_search_result<'a>(
    extensions_search_results: Vec<(
        Result<Vec<vouch_lib::extension::RegistryPackageMetadata>>,
        &'a Box<dyn vouch_lib::extension::Extension>,
    )>,
) -> Result<Vec<vouch_lib::extension::RegistryPackageMetadata>> {
    let mut selection = Err(format_err!(
        "Extensions have failed to find package in package registries."
    ));
    let mut ok_extension_names = Vec::<_>::new();

    for (search_result, extension) in extensions_search_results.into_iter() {
        if search_result.is_err() {
            continue;
        }

        ok_extension_names.push(extension.name());
        selection = search_result;
    }

    if ok_extension_names.len() > 1 {
        Err(format_err!(
            "Found multiple matching candidate packages.\n\
        Please specify an extension using --extension (-e).\n\
        Matching extensions: {}",
            ok_extension_names.join(", ")
        ))
    } else {
        selection
    }
}

/// Identify all supported dependencies in a local code base.
///
/// Conducts a parallel search across extensions.
pub fn identify_local_dependencies(
    extensions: &Vec<Box<dyn vouch_lib::extension::Extension>>,
    working_directory: &std::path::PathBuf,
) -> Result<Vec<Result<Vec<vouch_lib::extension::DependenciesSpec>>>> {
    crossbeam_utils::thread::scope(|s| {
        let mut threads = Vec::new();
        for extension in extensions {
            threads
                .push(s.spawn(move |_| extension.identify_local_dependencies(&working_directory)));
        }
        let mut result = Vec::new();
        for thread in threads {
            result.push(thread.join().unwrap());
        }
        Ok(result)
    })
    .unwrap()
}
