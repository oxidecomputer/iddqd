use core::hash::{BuildHasher, Hash};
use debug_ignore::DebugIgnore;
use derive_where::derive_where;

/// Packages up a state and a hash for later validation.
#[derive_where(Debug)]
#[derive(Clone)]
pub(crate) struct MapHash<S> {
    pub(super) state: DebugIgnore<S>,
    pub(super) hash: u64,
}

impl<S: BuildHasher> MapHash<S> {
    pub(crate) fn is_same_hash<K: Hash>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
