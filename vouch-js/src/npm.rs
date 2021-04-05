use anyhow::{format_err, Context, Result};
use std::collections::HashSet;

struct ParsedVersion {
    version: Option<String>,
    parse_error: bool,
    missing: bool,
}

static HOST_NAME: &str = "npmjs.com";

/// Parse and clean package version string.
///
/// Returns a structure which details common errors.
fn get_parsed_version(version: &Option<&str>) -> Result<ParsedVersion> {
    Ok(ParsedVersion {
        version: version.and_then(|v| Some(v.to_string())),
        parse_error: false,
        missing: version.is_none(),
    })
}

fn parse_section(
    json_section: &serde_json::map::Map<std::string::String, serde_json::value::Value>,
) -> Result<HashSet<vouch_lib::extension::LocalDependency>> {
    let mut dependencies = HashSet::new();
    for (package_name, entry) in json_section {
        let version_parse_result = get_parsed_version(&entry["version"].as_str())?;

        dependencies.insert(vouch_lib::extension::LocalDependency {
            registry_host_name: HOST_NAME.to_owned(),
            name: package_name.clone(),
            version: version_parse_result.version,
            version_parse_error: version_parse_result.parse_error,
            missing_version: version_parse_result.missing,
        });
    }
    Ok(dependencies)
}

/// Parse dependencies from project dependencies definition file.
pub fn get_dependencies(
    file_path: &std::path::PathBuf,
) -> Result<HashSet<vouch_lib::extension::LocalDependency>> {
    let file = std::fs::File::open(file_path)?;
    let reader = std::io::BufReader::new(file);
    let package_json_file: serde_json::Value = serde_json::from_reader(reader).context(format!(
        "Failed to parse package-lock.json: {}",
        file_path.display()
    ))?;

    let mut all_dependencies: HashSet<vouch_lib::extension::LocalDependency> = HashSet::new();
    for section in vec!["dependencies"] {
        let json_section = package_json_file[section].as_object().ok_or(format_err!(
            "Failed to parse '{}' section of package-lock.json file",
            section
        ))?;
        let dependencies = parse_section(&json_section)?;
        for dependency in dependencies {
            all_dependencies.insert(dependency);
        }
    }
    Ok(all_dependencies)
}
