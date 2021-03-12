use anyhow::{Context, Result};
use std::io::Write;

use crate::package;
use crate::review;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub line: u64,
    pub character: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum Summary {
    Pass,
    Warn,
    Fail,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub file: std::path::PathBuf,
    pub summary: Summary,
    pub description: String,
    pub selection: Selection,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetailedReview {
    pub title: String,
    pub description: String,
    pub is_primary: Option<bool>,
    pub comments: Vec<Comment>,
}

pub fn add_empty(package: &package::Package, reviews_directory: &std::path::PathBuf) -> Result<()> {
    let detailed_review = review::DetailedReview {
        title: "local".to_string(),
        description: format!("Package name-version: {}-{}", package.name, package.version),
        is_primary: Some(true),
        comments: Vec::new(),
    };

    let review_file_path = reviews_directory.join("local.review");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(&review_file_path)
        .context(format!(
            "Can't open/create file for writing: {}",
            review_file_path.display()
        ))?;
    file.write_all(serde_json::to_string_pretty(&detailed_review)?.as_bytes())?;
    Ok(())
}
