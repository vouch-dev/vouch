use std::hash::Hash;

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Registry {
    #[serde(skip)]
    pub id: crate::common::index::ID,
    pub host_name: String,
    pub registry_human_url: url::Url,
    pub archive_url: url::Url,
}

impl crate::common::HashSansId for Registry {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.host_name.hash(state);
        self.registry_human_url.hash(state);
        self.archive_url.hash(state);
    }
}

impl crate::common::index::Identify for Registry {
    fn id(&self) -> crate::common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut crate::common::index::ID {
        &mut self.id
    }
}
