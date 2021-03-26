//! A module for data structures which are available to all super modules.
//!
//! This module contains data structures which are available to all super modules.
//! The number of data structures in this module should be minimized. The data structures
//! should be as simple as possible.
//!
//! Print statements are prohibited whithin this module. Logging is allowed.

use serde;
use std::hash::Hash;

mod package_security;
pub mod rating;
mod review_confidence;

pub use package_security::PackageSecurity;
pub use review_confidence::ReviewConfidence;

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Review {
    #[serde(skip)]
    pub id: crate::common::index::ID,

    #[serde(rename = "package-security")]
    pub package_security: PackageSecurity,

    #[serde(rename = "review-confidence")]
    pub review_confidence: ReviewConfidence,

    #[serde(skip)]
    pub peer: crate::peer::Peer,

    pub package: crate::package::Package,

    pub comments: Vec<crate::review::comment::Comment>,
}

impl crate::common::HashSansId for Review {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.package_security.hash(state);
        self.review_confidence.hash(state);
        self.peer.hash(state);
        self.package.hash(state);
        self.comments.hash(state);
    }
}
