use anyhow::{format_err, Context, Result};
use std::convert::TryFrom;

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct Core {
    #[serde(rename = "root-git-url")]
    pub root_git_url: Option<crate::common::GitUrl>,

    #[serde(rename = "notify-vouch-public-sync")]
    pub notify_vouch_public_sync: bool,
}

fn get_regex() -> Result<regex::Regex> {
    Ok(regex::Regex::new(r"core\.(.*)")?)
}

pub fn is_match(name: &str) -> Result<bool> {
    Ok(get_regex()?.is_match(name))
}

pub fn set(core: &mut Core, name: &str, value: &str) -> Result<()> {
    let name_error_message = format!("Unknown setting field name: {}", name);

    let captures = get_regex()?
        .captures(name)
        .ok_or(format_err!(name_error_message.clone()))?;
    let field = captures
        .get(1)
        .ok_or(format_err!(name_error_message.clone()))?
        .as_str();

    match field {
        "root-git-url" => {
            let url = crate::common::GitUrl::try_from(value)
                .context(format!("Failed to parse URL: {}", value))?;

            let paths = crate::common::fs::DataPaths::new()?;
            let repo = git2::Repository::open(&paths.root_directory)?;
            repo.remote_set_url("origin", &url.to_string())?;

            core.root_git_url = Some(crate::common::GitUrl::try_from(value)?);
            Ok(())
        }
        "notify-vouch-public-sync" => {
            let value = match value {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(format_err!(
                        "Expected value: `true` or `false`. Found: {}",
                        value
                    ));
                }
            };
            core.notify_vouch_public_sync = value;
            Ok(())
        }
        _ => Err(format_err!(name_error_message.clone())),
    }
}

pub fn get(core: &Core, name: &str) -> Result<String> {
    let name_error_message = format!("Unknown setting field name: {}", name);

    let captures = get_regex()?
        .captures(name)
        .ok_or(format_err!(name_error_message.clone()))?;
    let field = captures
        .get(1)
        .ok_or(format_err!(name_error_message.clone()))?
        .as_str();

    match field {
        "root-git-url" => Ok(match &core.root_git_url {
            Some(url) => url.to_string(),
            None => "".to_string(),
        }),
        "notify-vouch-public-sync" => Ok(core.notify_vouch_public_sync.to_string()),
        _ => Err(format_err!(name_error_message.clone())),
    }
}
