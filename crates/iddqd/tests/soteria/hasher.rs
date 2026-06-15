//! Hashers used by the public-API proofs.

/// A deterministic, correctly behaving (lawful) hasher: a pure function of the
/// bytes written.
///
/// This doesn't use random state, which Soteria cannot execute.
#[derive(Clone, Default)]
pub(crate) struct LawfulHasher;

impl core::hash::BuildHasher for LawfulHasher {
    type Hasher = LawfulHasherState;
    fn build_hasher(&self) -> LawfulHasherState {
        LawfulHasherState(0)
    }
}

pub(crate) struct LawfulHasherState(u64);

impl core::hash::Hasher for LawfulHasherState {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut acc = self.0;
        for &b in bytes {
            acc = acc.wrapping_mul(31).wrapping_add(b as u64);
        }
        self.0 = acc;
    }
}

/// A buggy (lawless) hasher: `finish` returns a fresh nondeterministic value on
/// every call, so even equal keys may hash differently from one call to the
/// next.
///
/// The symbolic stand-in for arbitrarily buggy user `Hash` code
/// (non-deterministic, inconsistent with `Eq`, etc., though this doesn't
/// simulate panics).
#[derive(Clone, Default)]
pub(crate) struct LawlessHasher;

impl core::hash::BuildHasher for LawlessHasher {
    type Hasher = NondetHasher;
    fn build_hasher(&self) -> NondetHasher {
        NondetHasher
    }
}

pub(crate) struct NondetHasher;

impl core::hash::Hasher for NondetHasher {
    fn finish(&self) -> u64 {
        // A fresh symbolic value per call. This is maximally adversarial.
        soteria::nondet_bytes()
    }
    fn write(&mut self, _bytes: &[u8]) {}
}
