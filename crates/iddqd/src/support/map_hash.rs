use core::{
    fmt,
    hash::{BuildHasher, Hash},
};

/// Packages up a hash for later validation.
#[derive(Clone)]
pub(crate) struct MapHash {
    hash: u64,
}

impl MapHash {
    /// Creates a new `MapHash` from a key.
    #[cfg(not(soteria))]
    #[expect(
        clippy::disallowed_methods,
        reason = "the only place BuildHasher::hash_one is allowed"
    )]
    pub(crate) fn compute<S: BuildHasher, K: Hash>(state: &S, key: K) -> Self {
        Self { hash: state.hash_one(key) }
    }

    // Soteria (2026-10-01) replaces `BuildHasher::hash_one` with a stub that
    // always returns 0, which would hide adversarial hashers from the proofs.
    // This works, though. This has been reported to one of the Soteria
    // maintainers.
    #[cfg(soteria)]
    pub(crate) fn compute<S: BuildHasher, K: Hash>(state: &S, key: K) -> Self {
        use core::hash::Hasher;

        let mut hasher = state.build_hasher();
        key.hash(&mut hasher);
        Self { hash: hasher.finish() }
    }

    pub(super) fn hash(&self) -> u64 {
        self.hash
    }

    pub(crate) fn is_same_hash<S: BuildHasher, K: Hash>(
        &self,
        state: &S,
        key: K,
    ) -> bool {
        self.hash == Self::compute(state, key).hash
    }
}

impl fmt::Debug for MapHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapHash")
            .field("hash", &self.hash)
            .finish_non_exhaustive()
    }
}
