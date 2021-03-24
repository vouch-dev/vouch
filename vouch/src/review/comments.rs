use anyhow::{format_err, Result};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Position {
    pub line: u64,
    pub character: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Summary {
    Pass,
    Warn,
    Fail,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Comment {
    #[serde(rename = "file")]
    pub path: std::path::PathBuf,
    pub summary: Summary,
    pub description: String,
    pub selection: Option<Selection>,
}

fn strip_top_directory(path: &mut std::path::PathBuf) -> Result<()> {
    let top_directory = path.iter().next().ok_or(format_err!(
        "Review contains comment with empty path field."
    ))?;
    *path = path.strip_prefix(top_directory)?.to_path_buf();
    Ok(())
}

pub fn clean(comments: &mut Vec<Comment>) -> Result<()> {
    for comment in comments.iter_mut() {
        strip_top_directory(&mut comment.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_top_directory() -> Result<()> {
        let mut path = std::path::PathBuf::from("top/middle/local.review");
        strip_top_directory(&mut path)?;

        let result = path;
        let expected = std::path::PathBuf::from("middle/local.review");
        assert_eq!(result, expected);
        Ok(())
    }
}
