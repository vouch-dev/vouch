use anyhow::{format_err, Context, Result};
use std::collections::HashSet;

struct ParsedVersion {
    version: Option<String>,
    parse_error: bool,
    missing: bool,
}

/// Parse and clean package version string.
///
/// Returns a structure which details common errors.
fn get_parsed_version(version: &Option<&str>) -> Result<ParsedVersion> {
    let cleaned_version = match version {
        Some(v) => match v.strip_prefix("==") {
            Some(v) => v,
            None => {
                return Ok(ParsedVersion {
                    version: version.and_then(|v| Some(v.to_string())),
                    parse_error: true,
                    missing: false,
                });
            }
        },
        None => {
            return Ok(ParsedVersion {
                version: version.and_then(|v| Some(v.to_string())),
                parse_error: true,
                missing: true,
            });
        }
    };
    Ok(ParsedVersion {
        version: Some(cleaned_version.to_string()),
        parse_error: false,
        missing: false,
    })
}

fn parse_section(
    json_section: &serde_json::map::Map<std::string::String, serde_json::value::Value>,
) -> Result<HashSet<vouch_lib::extension::LocalDependancy>> {
    let mut dependancies = HashSet::new();
    for (package_name, entry) in json_section {
        let version_parse_result = get_parsed_version(&entry["version"].as_str())?;

        dependancies.insert(vouch_lib::extension::LocalDependancy {
            name: package_name.clone(),
            version: version_parse_result.version,
            version_parse_error: version_parse_result.parse_error,
            missing_version: version_parse_result.missing,
        });
    }
    Ok(dependancies)
}

/// Parse dependancies from pip lock file (Pipfile.lock).
pub fn get_dependancies(
    file_path: &std::path::PathBuf,
) -> Result<HashSet<vouch_lib::extension::LocalDependancy>> {
    let file = std::fs::File::open(file_path)?;
    let reader = std::io::BufReader::new(file);
    let pipfile: serde_json::Value = serde_json::from_reader(reader).context(format!(
        "Failed to parse Pipfile.lock: {}",
        file_path.display()
    ))?;

    let mut all_dependancies: HashSet<vouch_lib::extension::LocalDependancy> = HashSet::new();
    for section in vec!["default", "develop"] {
        let json_section = pipfile[section].as_object().ok_or(format_err!(
            "Failed to parse '{}' section of Pipfile.lock",
            section
        ))?;
        let dependancies = parse_section(&json_section)?;
        for dependancy in dependancies {
            all_dependancies.insert(dependancy);
        }
    }
    Ok(all_dependancies)
}
