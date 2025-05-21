use core::hash::{BuildHasher, Hash};
use hashbrown::DefaultHashBuilder;

pub(crate) type HashState = DefaultHashBuilder;

/// Packages up a state and a hash for later validation.
#[derive(Clone, Debug)]
pub(crate) struct MapHash {
    pub(super) state: HashState,
    pub(super) hash: u64,
}

impl MapHash {
    pub(crate) fn is_same_hash<K: Hash>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
