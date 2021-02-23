use anyhow::{format_err, Result};
use crossbeam_utils;
use std::collections::{BTreeSet, HashMap};
use vouch_lib::extension::Extension;

use crate::common;

static EXTENSION_FILE_NAME_PREFIX: &str = "vouch-";

fn parallel_search_extensions(
    package_name: &str,
    package_version: &str,
    extensions: &Vec<Box<dyn vouch_lib::extension::Extension>>,
    working_directory: &std::path::PathBuf,
) -> Result<Vec<Result<vouch_lib::extension::RemotePackageMetadata>>> {
    // TODO: Switch to message passing over channels. Low confidence, high confidence results, etc.
    crossbeam_utils::thread::scope(|s| {
        let mut threads = Vec::new();
        for extension in extensions {
            threads.push(s.spawn(move |_| {
                extension.remote_package_metadata(
                    &package_name,
                    &package_version,
                    &working_directory,
                )
            }));
        }
        let mut result = Vec::new();
        for thread in threads {
            result.push(thread.join().unwrap());
        }
        Ok(result)
    })
    .unwrap()
}

/// Search all extension remote data registries for package metadata.
///
/// Inspect local file system for clues to narrow down the search_remote_metadata.
/// This is public because the return value is useful for functionality in the repositories module.
pub fn get_remote_package_metadata<'a>(
    package_name: &str,
    package_version: &str,
    extensions: &'a Vec<Box<dyn vouch_lib::extension::Extension>>,
) -> Result<
    Option<(
        &'a Box<dyn vouch_lib::extension::Extension>,
        vouch_lib::extension::RemotePackageMetadata,
    )>,
> {
    let working_directory = std::env::current_dir()?;
    let search_results = parallel_search_extensions(
        &package_name,
        &package_version,
        &extensions,
        &working_directory,
    )?;

    // TODO: Allow user to select between multiple found results.
    // Select first plausable result.
    for (search_result, extension) in search_results.into_iter().zip(extensions.iter()) {
        match search_result {
            Ok(search_result) => {
                return Ok(Some((extension, search_result)));
            }
            Err(_) => continue,
        };
    }
    Ok(None)
}

/// Identify all supported dependancies in a local code base.
///
/// Conducts a parallel search accross extensions.
pub fn identify_local_dependancies(
    extensions: &Vec<Box<dyn vouch_lib::extension::Extension>>,
    working_directory: &std::path::PathBuf,
) -> Result<Vec<Result<Vec<vouch_lib::extension::LocalDependancy>>>> {
    crossbeam_utils::thread::scope(|s| {
        let mut threads = Vec::new();
        for extension in extensions {
            threads
                .push(s.spawn(move |_| extension.identify_local_dependancies(&working_directory)));
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

    let process_extensions = get_process_extensions()?;
    for extension in process_extensions.into_iter() {
        all_extensions.push(Box::new(extension) as Box<dyn vouch_lib::extension::Extension>);
    }

    Ok(all_extensions)
}

/// Returns enabled extensions.
pub fn get_enabled_extensions(
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
        .collect();

    Ok(extensions)
}

/// Update config with current set of extensions.
pub fn update_config() -> Result<()> {
    let mut config = common::config::Config::load()?;
    let extensions = get_all_extensions()?;

    let configured_names: std::collections::BTreeSet<String> = config
        .extensions
        .enabled
        .keys()
        .map(|name| name.clone())
        .collect();
    let all_found_names: std::collections::BTreeSet<String> = extensions
        .iter()
        .map(|extension| extension.name())
        .collect();

    let stale_config_names: Vec<_> = configured_names.difference(&all_found_names).collect();
    for name in &stale_config_names {
        config.extensions.enabled.remove(name.clone());
    }

    let newly_found_names: Vec<_> = all_found_names.difference(&configured_names).collect();
    for name in &newly_found_names {
        config.extensions.enabled.insert((*name).clone(), true);
    }

    if !stale_config_names.is_empty() || !newly_found_names.is_empty() {
        config.dump()?;
    }
    Ok(())
}

fn get_extension_paths() -> Result<HashMap<String, std::path::PathBuf>> {
    let mut result: HashMap<String, std::path::PathBuf> = HashMap::new();

    let env_path_value =
        std::env::var_os("PATH").ok_or(format_err!("Failed to read PATH enviroment variable."))?;
    let paths = std::env::split_paths(&env_path_value);

    for path in paths {
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

fn get_extension_config_path(extension_name: &str) -> Result<std::path::PathBuf> {
    let config_paths = crate::common::fs::ConfigPaths::new()?;
    Ok(config_paths.extensions_directory.join(format!(
        "{extension_name}.yaml",
        extension_name = extension_name
    )))
}
