use anyhow::{format_err, Context, Result};

/// Setup workspace for vscode review.
pub fn setup(workspace_directory: &std::path::PathBuf) -> Result<std::path::PathBuf> {
    let vscode_review_directory = workspace_directory.join(".vscode").join("reviews");
    std::fs::create_dir_all(&vscode_review_directory).context(format!(
        "Can't create directory: {}",
        vscode_review_directory.display()
    ))?;
    Ok(vscode_review_directory)
}

pub fn run(workspace_directory: &std::path::PathBuf) -> Result<()> {
    let mut child = std::process::Command::new("code")
        .args(vec![
            "--wait",
            "--new-window",
            workspace_directory.to_str().ok_or(format_err!(
                "Failed to convert PathBuf to str: {}",
                workspace_directory.display()
            ))?,
        ])
        .current_dir(workspace_directory)
        .spawn()
        .expect("Failed to start vscode.");
    let _result = child.wait()?;
    Ok(())
}
