//! IdHashMap proofs.

use crate::{
    hasher::{LawfulHasher, LawlessHasher},
    params::{SEQ_KEYS, SEQ_OPS, nondet_u8_below},
};
use iddqd::{IdHashItem, IdHashMap, internal::ValidateCompact};

/// A minimal [`IdHashItem`]: a `u8` key with a `u32` value.
#[derive(Debug)]
struct Item {
    key: u8,
    value: u32,
}

impl IdHashItem for Item {
    type Key<'a> = u8;
    fn key(&self) -> u8 {
        self.key
    }
    fn upcast_key<'short, 'long: 'short>(long: u8) -> u8 {
        long
    }
}

/// With a correctly behaving hasher, an [`IdHashMap`] built on the reference
/// table works correctly.
///
/// This is a smoke test that doesn't do any symbolic execution -- all it does is
/// validate that `cfg(soteria)` works.
#[test]
fn lawful_roundtrip() {
    let mut map: IdHashMap<Item, LawfulHasher> =
        IdHashMap::with_hasher(LawfulHasher);
    let _ = map.insert_unique(Item { key: 1, value: 10 });
    let _ = map.insert_unique(Item { key: 2, value: 20 });

    soteria::assert(
        map.get(&1u8).map(|i| i.value) == Some(10),
        "key 1 maps to its value",
    );
    soteria::assert(
        map.get(&2u8).map(|i| i.value) == Some(20),
        "key 2 maps to its value",
    );
    soteria::assert(map.get(&3u8).is_none(), "an absent key is not found");

    let dup = map.insert_unique(Item { key: 1, value: 99 });
    soteria::assert(dup.is_err(), "a duplicate key is rejected");

    map.validate(ValidateCompact::Compact)
        .expect("two distinct keys form a compact, valid map");
}

/// Structural soundness across *arbitrary operation sequences* under an
/// arbitrarily adversarial (lawless) hash.
///
/// This is the proof analog of the model-based `proptest_ops` test.
///
/// # Notes
///
/// We don't cover `insert_overwrite` here since that will panic under
/// adversarial input. We could catch the panic here but that slows down
/// proof execution tremendously. So instead, we have a separate proof
/// for `insert_overwrite` below.
///
/// We only call `validate_structural`, not full `validate`, since under
/// an adversarial hash we can end up not finding items by their key. Only
/// structural validity is required to prevent unsoundness.
#[test]
fn lawless_operation_sequence() {
    let mut map: IdHashMap<Item, LawlessHasher> =
        IdHashMap::with_hasher(LawlessHasher);

    for _ in 0..SEQ_OPS {
        let key = nondet_u8_below(SEQ_KEYS);
        let op: u8 = soteria::nondet_bytes();
        soteria::assume(op < 2);

        match op {
            0 => {
                let _ = map.insert_unique(Item { key, value: 0 });
            }
            // op == 1
            _ => {
                let _ = map.remove(&key);
            }
        }

        map.validate_structural(ValidateCompact::NonCompact).expect(
            "the map stays structurally sound after every op under any hash",
        );
    }

    // Drop is quite slow under Soteria and not relevant to soundness today.
    // Since we set `--ignore-leaks`, we can skip it.
    //
    // (Drop might become relevant to soundness in the future, in which case we
    // should cover it here.)
    std::mem::forget(map);
}

/// `insert_overwrite` is structurally sound under a lawless hash, whether it
/// completes or trips iddqd's own `is_same_hash` fail-fast guard (which the
/// lawless hash induces by disagreeing on the recomputed key hash).
///
/// Note that this isn't proving panic safety in general, only that an
/// `insert_overwrite` panic leaves the map in a valid state. For panic safety,
/// see the corresponding model-based tests.
#[test]
fn overwrite_fail_fast_is_sound() {
    let mut map: IdHashMap<Item, LawlessHasher> =
        IdHashMap::with_hasher(LawlessHasher);
    let k = nondet_u8_below(2);

    let _ = map.insert_unique(Item { key: k, value: 0 });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = map.insert_overwrite(Item { key: k, value: 1 });
    }));

    map.validate_structural(ValidateCompact::NonCompact).expect(
        "sound whether insert_overwrite completed or fail-fast panicked",
    );
    std::mem::forget(map);
}
