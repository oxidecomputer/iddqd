use core::{
    fmt,
    hash::{BuildHasher, Hash},
};

/// Packages up a state and a hash for later validation.
#[derive(Clone)]
pub(crate) struct MapHash<S> {
    pub(super) state: S,
    pub(super) hash: u64,
}

impl<S> fmt::Debug for MapHash<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapHash")
            .field("hash", &self.hash)
            .finish_non_exhaustive()
    }
}

impl<S: BuildHasher> MapHash<S> {
    #[inline]
    pub(crate) fn build_hasher(&self) -> S::Hasher {
        self.state.build_hasher()
    }

    #[inline]
    pub(crate) fn hash(&self) -> u64 {
        self.hash
    }

    pub(crate) fn is_same_hash<K: Hash>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
