use std::hash::{BuildHasher, Hash, RandomState};

/// Packages up a state and a hash for later validation.
#[derive(Clone, Debug)]
pub(crate) struct MapHash {
    pub(super) state: RandomState,
    pub(super) hash: u64,
}

impl MapHash {
    pub(crate) fn is_same_hash<K: Hash>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
