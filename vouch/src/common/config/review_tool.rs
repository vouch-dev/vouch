use anyhow::{format_err, Result};

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct ReviewTool {
    pub name: String,

    #[serde(rename = "install-check")]
    pub install_check: bool,
}

fn get_regex() -> Result<regex::Regex> {
    Ok(regex::Regex::new(r"review-tool\.(.*)")?)
}

pub fn is_match(name: &str) -> Result<bool> {
    Ok(get_regex()?.is_match(name))
}

pub fn set(review_tool: &mut ReviewTool, name: &str, value: &str) -> Result<()> {
    let name_error_message = format!("Unknown setting field name: {}", name);

    let captures = get_regex()?
        .captures(name)
        .ok_or(format_err!(name_error_message.clone()))?;
    let field = captures
        .get(1)
        .ok_or(format_err!(name_error_message.clone()))?
        .as_str();

    match field {
        "name" => {
            review_tool.name = value.to_string();
            Ok(())
        }
        "install-check" => {
            review_tool.install_check = value == "true";
            Ok(())
        }
        _ => Err(format_err!(name_error_message.clone())),
    }
}

pub fn get(review_tool: &ReviewTool, name: &str) -> Result<String> {
    let name_error_message = format!("Unknown setting field name: {}", name);

    let captures = get_regex()?
        .captures(name)
        .ok_or(format_err!(name_error_message.clone()))?;
    let field = captures
        .get(1)
        .ok_or(format_err!(name_error_message.clone()))?
        .as_str();

    match field {
        "name" => Ok(review_tool.name.to_string()),
        "install-check" => Ok(review_tool.install_check.to_string()),
        _ => Err(format_err!(name_error_message.clone())),
    }
}
