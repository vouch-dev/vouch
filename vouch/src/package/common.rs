use crate::common;
use crate::registry;
use std::hash::Hash;

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Package {
    #[serde(skip)]
    pub id: common::index::ID,

    pub name: String,
    pub version: String,
    pub registry: registry::Registry,
    pub artifact_hash: String,
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.name,
            &self.version,
            &self.registry,
            &self.artifact_hash,
            &self.id,
        )
            .cmp(&(
                &other.name,
                &other.version,
                &other.registry,
                &other.artifact_hash,
                &other.id,
            ))
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl common::index::Identify for Package {
    fn id(&self) -> common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut common::index::ID {
        &mut self.id
    }
}

impl crate::common::HashSansId for Package {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.version.hash(state);
        self.registry.hash_sans_id(state);
        self.artifact_hash.hash(state);
    }
}
