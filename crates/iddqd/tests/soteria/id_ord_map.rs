//! IdOrdMap proofs: a single ordered key per value, backed by a B-tree table
//! with an external comparator.
//!
//! Unlike the hash maps, the `cfg(soteria)` build keeps the *real* table under
//! proof — std `BTreeMap`, the thread-local comparator, the lifetime
//! `transmute`, and the `AtomicU32` index cells. At these scales the B-tree
//! never exceeds a single leaf node, so Soteria can execute it directly.

use crate::params::{SEQ_KEYS, SEQ_OPS, nondet_u8_below};
use core::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};
use iddqd::{
    IdOrdItem, IdOrdMap,
    internal::{ValidateChaos, ValidateCompact},
};

/// A minimal, lawfully-ordered [`IdOrdItem`]: a `u8` key with a `u32` value.
#[derive(Debug)]
struct LawfulItem {
    key: u8,
    value: u32,
}

impl IdOrdItem for LawfulItem {
    type Key<'a> = u8;
    fn key(&self) -> u8 {
        self.key
    }
    fn upcast_key<'short, 'long: 'short>(long: u8) -> u8 {
        long
    }
}

/// A key whose `Ord` is arbitrarily adversarial (lawless): every comparison
/// returns a fresh nondeterministic `Ordering`, so the order is non-reflexive,
/// non-transitive, and inconsistent from one call to the next.
///
/// This is the symbolic stand-in for arbitrarily buggy user `Ord` code, and the
/// `Ord` analog of [`LawlessHasher`](crate::hasher::LawlessHasher). `Hash`
/// stays lawful: the change-detection hash the map caches is not part of what
/// we're proving here (we already have tests for that).
#[derive(Clone, Copy, Debug)]
struct LawlessKey(u8);

impl PartialEq for LawlessKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for LawlessKey {}

impl PartialOrd for LawlessKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LawlessKey {
    fn cmp(&self, _other: &Self) -> Ordering {
        // A fresh symbolic ordering per call. This is maximally adversarial.
        let n: u8 = soteria::nondet_bytes();
        soteria::assume(n < 3);
        match n {
            0 => Ordering::Less,
            1 => Ordering::Equal,
            _ => Ordering::Greater,
        }
    }
}

impl Hash for LawlessKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// A lawlessly-ordered [`IdOrdItem`], keyed by [`LawlessKey`].
#[derive(Debug)]
struct LawlessItem {
    key: u8,
    value: u32,
}

impl IdOrdItem for LawlessItem {
    type Key<'a> = LawlessKey;
    fn key(&self) -> LawlessKey {
        LawlessKey(self.key)
    }
    fn upcast_key<'short, 'long: 'short>(long: LawlessKey) -> LawlessKey {
        long
    }
}

/// With a correctly behaving `Ord`, an [`IdOrdMap`] on the real B-tree table
/// works correctly.
///
/// This is a smoke test that does minimal symbolic execution -- all it does is
/// validate that `cfg(soteria)` works against the real std `BTreeMap`.
#[test]
fn lawful_roundtrip() {
    let mut map: IdOrdMap<LawfulItem> = IdOrdMap::new();
    let _ = map.insert_unique(LawfulItem { key: 1, value: 10 });
    let _ = map.insert_unique(LawfulItem { key: 2, value: 20 });

    soteria::assert(
        map.get(&1u8).map(|i| i.value) == Some(10),
        "key 1 maps to its value",
    );
    soteria::assert(
        map.get(&2u8).map(|i| i.value) == Some(20),
        "key 2 maps to its value",
    );
    soteria::assert(map.get(&3u8).is_none(), "an absent key is not found");

    let dup = map.insert_unique(LawfulItem { key: 1, value: 99 });
    soteria::assert(dup.is_err(), "a duplicate key is rejected");

    map.validate(ValidateCompact::Compact, ValidateChaos::No)
        .expect("two distinct keys form a compact, valid map");
}

/// Structural soundness across *arbitrary operation sequences* under an
/// arbitrarily adversarial (lawless) `Ord`.
///
/// This is the proof analog of the model-based `proptest_ops` test, and the
/// `Ord` counterpart of `id_hash_map::lawless_operation_sequence`. The
/// load-bearing defense is the physical-index tiebreaker in the B-tree
/// comparator (`then_with(|| a.cmp(&b))`): distinct indexes must never compare
/// equal, no matter how adversarial the user `Ord` is, or the unsafe code could
/// hand out mutable aliases.
///
/// # Notes
///
/// We don't cover `insert_overwrite` here since its occupied arm can still
/// panic under a lawless `Ord` (`replace_at_index`'s key-equality check,
/// reached because `LawlessKey`'s `Eq` delegates to the lawless `cmp`). We
/// could catch the panic here but that slows down proof execution
/// tremendously. So instead, we have a separate proof for
/// `insert_overwrite` below.
///
/// We only call `validate_structural`, not full `validate`, since under
/// an adversarial `Ord` we can end up not finding items by their key. Only
/// structural validity is required to prevent unsoundness.
#[test]
fn lawless_operation_sequence() {
    let mut map: IdOrdMap<LawlessItem> = IdOrdMap::new();

    for _ in 0..SEQ_OPS {
        let key = nondet_u8_below(SEQ_KEYS);
        let op: u8 = soteria::nondet_bytes();
        soteria::assume(op < 2);

        match op {
            0 => {
                let _ = map.insert_unique(LawlessItem { key, value: 0 });
            }
            // op == 1
            _ => {
                let _ = map.remove(&LawlessKey(key));
            }
        }

        map.validate_structural(ValidateCompact::NonCompact).expect(
            "the map stays structurally sound after every op under any Ord",
        );
    }

    // Drop is quite slow under Soteria and not relevant to soundness today.
    // Since we set `--ignore-leaks`, we can skip it.
    //
    // (Drop might become relevant to soundness in the future, in which case we
    // should cover it here.)
    std::mem::forget(map);
}

/// Prove that `insert_overwrite` is structurally sound under a lawless
/// `Ord`, whichever arm the lawless lookup steers it into:
///
/// * If an existing item is found, an in-place replace. This arm can still
///   fail fast: `replace_at_index` re-checks key equality, and
///   `LawlessKey`'s `Eq` delegates to the lawless `cmp`. This is why this
///   proof keeps `catch_unwind` while the hash-map proofs (whose key `Eq`
///   is lawful) dropped it.
/// * If no item is found, an unchecked insert that may create a logical
///   duplicate. This arm no longer panics, but note that the B-tree
///   comparator's index tiebreaker keeps the two entries structurally distinct
///   (so structural soundness is preserved).
///
/// Note that this isn't proving panic safety in general, only that an
/// `insert_overwrite` panic leaves the map in a valid state. For panic
/// safety, see the corresponding model-based tests.
#[test]
fn overwrite_fail_fast_is_sound() {
    let mut map: IdOrdMap<LawlessItem> = IdOrdMap::new();
    let k = nondet_u8_below(SEQ_KEYS);

    let _ = map.insert_unique(LawlessItem { key: k, value: 0 });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = map.insert_overwrite(LawlessItem { key: k, value: 1 });
    }));

    map.validate_structural(ValidateCompact::NonCompact).expect(
        "sound whether insert_overwrite completed or fail-fast panicked",
    );
    std::mem::forget(map);
}
