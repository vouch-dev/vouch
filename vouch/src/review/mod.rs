use crate::common::StoreTransaction;
use anyhow::Result;

mod common;
pub mod detailed;
pub mod fs;
pub mod index;
pub mod tool;

pub use crate::review::common::{PackageSecurity, Review, ReviewConfidence};
pub use crate::review::detailed::DetailedReview;

pub fn store(review: &Review, tx: &StoreTransaction) -> Result<()> {
    // TODO: Enforce assumption in code: review already inserted in index.
    index::update(&review, &tx)?;
    fs::add(&review, None)?;
    Ok(())
}
