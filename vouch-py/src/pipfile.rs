use anyhow::{format_err, Context, Result};
use std::collections::HashSet;

static HOST_NAME: &str = "pypi.org";

/// Parse and clean package version string.
///
/// Returns a structure which details common errors.
fn get_parsed_version(version: &Option<&str>) -> vouch_lib::extension::common::VersionParseResult {
    let cleaned_version = match version {
        Some(v) => match v.strip_prefix("==") {
            Some(v) => v,
            None => {
                return Err(vouch_lib::extension::common::VersionError::from_parse_error(v));
            }
        },
        None => {
            return Err(vouch_lib::extension::common::VersionError::from_missing_version());
        }
    };
    Ok(cleaned_version.to_string())
}

fn parse_section(
    json_section: &serde_json::map::Map<std::string::String, serde_json::value::Value>,
) -> Result<HashSet<vouch_lib::extension::Dependency>> {
    let mut dependencies = HashSet::new();
    for (package_name, entry) in json_section {
        let version_parse_result = get_parsed_version(&entry["version"].as_str());

        dependencies.insert(vouch_lib::extension::Dependency {
            name: package_name.clone(),
            version: version_parse_result,
        });
    }
    Ok(dependencies)
}

/// Parse dependencies from project dependencies definition file.
pub fn get_dependencies(
    file_path: &std::path::PathBuf,
) -> Result<HashSet<vouch_lib::extension::Dependency>> {
    let file = std::fs::File::open(file_path)?;
    let reader = std::io::BufReader::new(file);
    let pipfile: serde_json::Value = serde_json::from_reader(reader).context(format!(
        "Failed to parse Pipfile.lock: {}",
        file_path.display()
    ))?;

    let mut all_dependencies: HashSet<vouch_lib::extension::Dependency> = HashSet::new();
    for section in vec!["default", "develop"] {
        let json_section = pipfile[section].as_object().ok_or(format_err!(
            "Failed to parse '{}' section of Pipfile.lock",
            section
        ))?;
        let dependencies = parse_section(&json_section)?;
        for dependency in dependencies {
            all_dependencies.insert(dependency);
        }
    }
    Ok(all_dependencies)
}

pub fn get_registry_host_name() -> String {
    HOST_NAME.to_string()
}
