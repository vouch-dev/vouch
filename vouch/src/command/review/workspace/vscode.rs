use anyhow::{Context, Result};

/// Setup workspace for vscode review.
pub fn setup(workspace_directory: &std::path::PathBuf) -> Result<std::path::PathBuf> {
    let vscode_review_directory = workspace_directory.join(".vscode").join("reviews");
    std::fs::create_dir_all(&vscode_review_directory).context(format!(
        "Can't create directory: {}",
        vscode_review_directory.display()
    ))?;
    Ok(vscode_review_directory)
}
