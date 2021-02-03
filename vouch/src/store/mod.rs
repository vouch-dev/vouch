use crate::common::StoreTransaction;
use crate::peer;
use anyhow::Result;

pub mod index;

pub struct Store {
    index: index::Index,
}

impl Store {
    /// Load root store.
    pub fn from_root() -> Result<Self> {
        Ok(Self {
            index: index::Index::from_root()?,
        })
    }

    /// Load the store of a given peer.
    pub fn from_peer(peer_subtree: &Vec<peer::Peer>) -> Result<Self> {
        Ok(Self {
            index: index::Index::from_peer(&peer_subtree)?,
        })
    }

    /// Load temporary storage. Useful for testing.
    #[allow(dead_code)]
    pub fn from_tmp() -> Result<Self> {
        let mut index = index::Index::in_memory()?;
        index::setup_in_memory(&mut index)?;
        Ok(Self { index })
    }

    pub fn get_transaction(&mut self) -> Result<StoreTransaction> {
        Ok(StoreTransaction::new(self.index.db.transaction()?)?)
    }
}
