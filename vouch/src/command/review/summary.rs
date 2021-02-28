use crate::review;
use anyhow::{format_err, Context, Result};
use std::io::Read;
use tempdir;

/// Validate and clean user input.
fn clean_user_input(edited_review: &str) -> Result<review::Review> {
    let edited_review: review::Review = serde_yaml::from_str(edited_review)?;

    if edited_review.package_security == review::PackageSecurity::Unset {
        return Err(format_err!(
            "Field `package-security` not set correctly. Example expected symtax: 1/5"
        ));
    }
    if edited_review.review_confidence == review::ReviewConfidence::Unset {
        return Err(format_err!(
            "Field `review-confidence` not set correctly. Example expected symtax: 1/5"
        ));
    }
    Ok(edited_review)
}

pub fn add_user_input(review: &review::Review) -> Result<review::Review> {
    log::debug!("Add user input to review.");

    let temporary_directory = tempdir::TempDir::new("vouch")?;
    let file_path = temporary_directory.path().join("user_edit_review.yaml");

    {
        let mut review = serde_yaml::to_string(&review)?;
        review = add_userguide_comments(&review)?;
        std::fs::write(&file_path, &review)?;
    }

    // TODO: This doesn't work whilst debugging. Find a solution.
    let editor = get_git_editor()?;
    std::process::Command::new(&editor)
        .arg(&file_path)
        .status()
        .expect(
            format!(
                "Failed to open review for user editing. Attempted to use editor: {}",
                editor
            )
            .as_str(),
        );

    let mut file = std::fs::OpenOptions::new().read(true).open(&file_path)?;
    let mut file_contents = String::new();
    file.read_to_string(&mut file_contents)
        .context("Cannot read edited review from tmp directory.")?;
    let mut edited_review = clean_user_input(&file_contents)?;
    edited_review.id = review.id;
    edited_review.peer = review.peer.clone();
    edited_review.package = review.package.clone();
    Ok(edited_review)
}

fn add_userguide_comments(review: &str) -> Result<String> {
    let mut review_and_comments = String::new();
    for line in review.lines() {
        if line.starts_with("package-security") {
            review_and_comments
                .push_str("# What are the security implications of using this package?\n");
            review_and_comments.push_str(
                "# (1) very dangerous, (2) dangerous, (3) neutral, (4) safe, (5) very safe\n",
            );
            review_and_comments.push_str("# Example: 3/5\n");
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
            review_and_comments.push_str("\n");
        } else if line.starts_with("review-confidence") {
            review_and_comments.push_str("# How confident are you in your review?\n");
            review_and_comments
                .push_str("# (1) very low, (2) low, (3) neutral, (4) high, (5) very high\n");
            review_and_comments.push_str("# Example: 3/5\n");
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
            review_and_comments.push_str("\n\n");
        } else {
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
        }
    }
    Ok(review_and_comments)
}

/// Returns the command line editor used by Git.
fn get_git_editor() -> Result<String> {
    Ok(match std::env::var("GIT_EDITOR") {
        Ok(k) => k,
        Err(_) => {
            std::env::var("EDITOR").context("Failed to obtain text editor for editing review.")?
        }
    })
}
