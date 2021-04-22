use anyhow::{format_err, Result};
use crossbeam_utils;
use std::collections::{BTreeSet, HashMap};
use vouch_lib::extension::{FromLib, FromProcess};

use crate::common;

pub mod manage;

pub static EXTENSION_FILE_NAME_PREFIX: &str = "vouch-";

/// Search package registries via extensions for package metadata from registries.
///
/// Raises errors for no results or multiple results. Ok for single result.
pub fn search<'a>(
    package_name: &str,
    package_version: &str,
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
        "Extensions have failed to find package in remote package registries."
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

/// Returns a set of all enabled installed extensions by names.
pub fn get_enabled_names(config: &common::config::Config) -> Result<BTreeSet<String>> {
    Ok(config
        .extensions
        .enabled
        .iter()
        .filter(|(_name, enabled_flag)| **enabled_flag)
        .map(|(name, _enabled_flag)| name.clone())
        .collect())
}

pub fn get_all_names(config: &common::config::Config) -> Result<BTreeSet<String>> {
    Ok(config
        .extensions
        .enabled
        .iter()
        .map(|(name, _enabled_flag)| name.clone())
        .collect())
}

/// Given an extension's name, returns true if the extension is enabled. Otherwise returns false.
pub fn is_enabled(name: &str, config: &common::config::Config) -> Result<bool> {
    Ok(*config.extensions.enabled.get(name).unwrap_or(&false))
}

/// Return handles to all known extensions.
pub fn get_all_extensions() -> Result<Vec<Box<dyn vouch_lib::extension::Extension>>> {
    log::debug!("Identifying all extensions.");

    let mut all_extensions = vec![
        Box::new(vouch_py_lib::PyExtension::new()) as Box<dyn vouch_lib::extension::Extension>,
        Box::new(vouch_js_lib::JsExtension::new()) as Box<dyn vouch_lib::extension::Extension>,
    ];

    for extension in get_process_extensions()? {
        all_extensions.push(Box::new(extension) as Box<dyn vouch_lib::extension::Extension>);
    }

    Ok(all_extensions)
}

/// Returns enabled extensions.
pub fn get_enabled_extensions(
    names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
) -> Result<Vec<Box<dyn vouch_lib::extension::Extension>>> {
    log::debug!("Identifying enabled extensions.");
    let extensions = get_all_extensions()?
        .into_iter()
        .filter(|extension| {
            *config
                .extensions
                .enabled
                .get(&extension.name())
                .unwrap_or(&false)
        })
        .filter(|extension| names.contains(&extension.name()))
        .collect();

    Ok(extensions)
}

/// Update config with current set of extensions.
pub fn update_config(config: &mut common::config::Config) -> Result<()> {
    log::debug!("Discover extensions and update config.");

    let extensions = get_all_extensions()?;
    let extension_name_map: std::collections::BTreeMap<_, _> = extensions
        .iter()
        .map(|extension| (extension.name(), extension))
        .collect();

    let all_found_names: std::collections::BTreeSet<_> =
        extension_name_map.keys().cloned().collect();

    let configured_names: std::collections::BTreeSet<_> = config
        .extensions
        .enabled
        .keys()
        .map(|name| name.clone())
        .collect();

    let stale_names: Vec<_> = configured_names.difference(&all_found_names).collect();
    let registries_map = config.extensions.registries.clone();
    for name in &stale_names {
        config.extensions.enabled.remove(name.clone());

        // Update registries map.
        for (registry, extension_name) in &registries_map {
            if *extension_name == **name {
                config.extensions.registries.remove(registry);
            }
        }
    }

    let new_names: Vec<_> = all_found_names.difference(&configured_names).collect();
    for name in &new_names {
        config.extensions.enabled.insert((**name).clone(), true);

        // Update registries map.
        if let Some(extension) = extension_name_map.get(name.as_str()) {
            for registry in extension.registries() {
                config
                    .extensions
                    .registries
                    .insert(registry, (*name).clone());
            }
        }
    }

    if !stale_names.is_empty() || !new_names.is_empty() {
        config.dump()?;
    }
    Ok(())
}

pub fn get_extension_paths() -> Result<HashMap<String, std::path::PathBuf>> {
    let mut result: HashMap<String, std::path::PathBuf> = HashMap::new();
    for path in get_candidate_extension_paths()? {
        // Skip non-valid paths.
        if !path.is_dir() && !path.is_file() {
            continue;
        }

        if path.is_file() {
            let name = match get_extension_name(&path)? {
                Some(name) => name,
                None => {
                    continue;
                }
            };
            result.insert(name, path);
            continue;
        }

        // Inspect file in directory. Does not investigate child directories.
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_file() {
                let name = match get_extension_name(&path)? {
                    Some(name) => name,
                    None => {
                        continue;
                    }
                };
                result.insert(name, path);
            }
        }
    }
    Ok(result)
}

fn get_candidate_extension_paths() -> Result<Vec<std::path::PathBuf>> {
    let env_path_value =
        std::env::var_os("PATH").ok_or(format_err!("Failed to read PATH environment variable."))?;
    let mut paths = std::env::split_paths(&env_path_value).collect::<Vec<_>>();

    if let Some(extensions_home_directory) = common::fs::get_extensions_default_directory() {
        if extensions_home_directory.exists() {
            paths.push(extensions_home_directory);
        }
    }
    Ok(paths)
}

fn get_extension_name(file_path: &std::path::PathBuf) -> Result<Option<String>> {
    let file_name = file_path
        .file_name()
        .ok_or(format_err!("Failed to parse path file name."))?
        .to_str()
        .ok_or(format_err!("Failed to parse path file name into string."))?
        .to_string();

    let captures = match regex::Regex::new(&format!(
        "{extension_file_name_prefix}([a-z]*).*",
        extension_file_name_prefix = EXTENSION_FILE_NAME_PREFIX
    ))?
    .captures(file_name.as_str())
    {
        Some(v) => v,
        None => {
            return Ok(None);
        }
    };

    let name = match captures.get(1) {
        Some(v) => v,
        None => {
            return Ok(None);
        }
    }
    .as_str();
    Ok(Some(name.to_string()))
}

/// Discovers and loads process extensions.
pub fn get_process_extensions() -> Result<Vec<vouch_lib::extension::process::ProcessExtension>> {
    let extension_paths = get_extension_paths()?;

    let mut threads = vec![];
    for (name, path) in extension_paths.iter() {
        let extension_config_path = get_extension_config_path(name)?;
        let process_path = path.clone();

        threads.push(std::thread::spawn(move || {
            vouch_lib::extension::process::ProcessExtension::from_process(
                &process_path,
                &extension_config_path,
            )
        }));
    }
    let extensions: Vec<Result<vouch_lib::extension::process::ProcessExtension>> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();

    let mut extension_map = HashMap::new();
    for ((_name, path), extension) in extension_paths.into_iter().zip(extensions.into_iter()) {
        extension_map.insert((*path).to_path_buf(), extension);
    }

    let mut valid_extensions = Vec::new();
    for (process_path, extension) in extension_map {
        match extension {
            Ok(v) => {
                valid_extensions.push(v);
            }
            Err(e) => {
                eprintln!(
                    "{extension_name}: Failed to load extension.\n{error}",
                    extension_name = process_path.display(),
                    error = e
                );
            }
        };
    }
    Ok(valid_extensions)
}

pub fn get_extension_config_path(extension_name: &str) -> Result<std::path::PathBuf> {
    let config_paths = crate::common::fs::ConfigPaths::new()?;
    Ok(config_paths.extensions_directory.join(format!(
        "{extension_name}.yaml",
        extension_name = extension_name
    )))
}

pub fn get_disabled_extension_names(config: &common::config::Config) -> Result<Vec<String>> {
    Ok(config
        .extensions
        .enabled
        .iter()
        .filter(|(_extension_name, enabled_flag)| **enabled_flag)
        .map(|(extension_name, _enabled_flag)| extension_name.clone())
        .collect::<Vec<_>>())
}

pub fn get_enabled_registry_host_names(
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
) -> Result<std::collections::HashSet<String>> {
    Ok(config
        .extensions
        .registries
        .iter()
        .filter(|(_host_name, extension_name)| extension_names.contains(*extension_name))
        .map(|(host_name, _extension_name)| host_name.clone())
        .collect())
}

/// Check given extensions are enabled. If not specified select all enabled extensions.
pub fn handle_extension_names_arg(
    extension_names: &Option<Vec<String>>,
    config: &common::config::Config,
) -> Result<BTreeSet<String>> {
    let names = match &extension_names {
        Some(extension_names) => {
            let disabled_names: Vec<_> = extension_names
                .iter()
                .cloned()
                .filter(|name| !is_enabled(&name, &config).unwrap_or(false))
                .collect();
            if !disabled_names.is_empty() {
                return Err(format_err!(
                    "The following disabled extensions were given: {}",
                    disabled_names.join(", ")
                ));
            } else {
                extension_names.into_iter().cloned().collect()
            }
        }
        None => get_enabled_names(&config)?,
    };
    log::debug!("Using extensions: {:?}", names);
    Ok(names)
}

/// Enable extension.
pub fn enable(name: &str, config: &mut common::config::Config) -> Result<()> {
    if let Some(enabled_status) = config.extensions.enabled.get_mut(&name.to_string()) {
        *enabled_status = true;
        config.dump()?;
        Ok(())
    } else {
        Err(format_err!("Failed to find extension."))
    }
}

/// Disable extension.
pub fn disable(name: &str, config: &mut common::config::Config) -> Result<()> {
    if let Some(enabled_status) = config.extensions.enabled.get_mut(&name.to_string()) {
        *enabled_status = false;
        config.dump()?;
        Ok(())
    } else {
        Err(format_err!("Failed to find extension."))
    }
}
