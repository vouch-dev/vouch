use anyhow::{format_err, Result};
mod vscode;

use crate::common;

pub fn check_install(config: &mut common::config::Config) -> Result<()> {
    // Skip check if previously passed.
    if config.review_tool.install_check {
        return Ok(());
    }
    if config.review_tool.name != "vscode" {
        return Err(format_err!(
            "Reviewing currently requires vscode. Unsupported review tool: {}",
            config.review_tool.name
        ));
    }
    vscode::setup()?;

    config.review_tool.install_check = true;
    config.dump()?;

    Ok(())
}

pub fn run(
    workspace_directory: &std::path::PathBuf,
    config: &common::config::Config,
) -> Result<()> {
    assert!(
        config.review_tool.install_check,
        "Attempted to run review tool whilst install check is false."
    );

    log::debug!("Running review tool.");
    vscode::run(&workspace_directory)?;
    log::debug!("Review tool exit complete.");
    Ok(())
}

/// Setup reviews directory within workspace.
pub fn ensure_reviews_directory(
    workspace_directory: &std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    let review_directory = vscode::setup_reviews_directory(&workspace_directory)?;
    Ok(review_directory)
}
