use anyhow::{format_err, Result};
pub mod common;
pub mod index;

pub use common::Comment;

fn strip_top_directory(path: &mut std::path::PathBuf) -> Result<()> {
    let top_directory = path.iter().next().ok_or(format_err!(
        "Review contains comment with empty path field."
    ))?;
    // *path = path.strip_prefix(top_directory)?.to_path_buf();
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
