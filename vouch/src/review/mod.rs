use crate::common::StoreTransaction;
use anyhow::Result;

pub mod active;
pub mod comment;
mod common;
pub mod fs;
pub mod index;
pub mod tool;
pub mod workspace;

pub use crate::review::common::{PackageSecurity, Review, ReviewConfidence};

pub fn store(review: &Review, tx: &StoreTransaction) -> Result<()> {
    index::update(&review, &tx)?;
    fs::add(&review)?;
    Ok(())
}
