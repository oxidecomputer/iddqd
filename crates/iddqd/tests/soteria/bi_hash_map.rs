use crate::hasher::{LawfulHasher, LawlessHasher};
use crate::params::{SEQ_KEYS, SEQ_OPS, nondet_u8_below};
use iddqd::{BiHashItem, BiHashMap, internal::ValidateCompact};

#[derive(Debug)]
struct BiItem {
    key1: u8,
    key2: u8,
    value: u32,
}

impl BiHashItem for BiItem {
    type K1<'a> = u8;
    type K2<'a> = u8;
    fn key1(&self) -> u8 {
        self.key1
    }
    fn key2(&self) -> u8 {
        self.key2
    }
    fn upcast_key1<'short, 'long: 'short>(long: u8) -> u8 {
        long
    }
    fn upcast_key2<'short, 'long: 'short>(long: u8) -> u8 {
        long
    }
}

#[test]
fn lawful_roundtrip() {
    let mut map: BiHashMap<BiItem, LawfulHasher> =
        BiHashMap::with_hasher(LawfulHasher);
    let _ = map.insert_unique(BiItem { key1: 1, key2: 10, value: 100 });
    let _ = map.insert_unique(BiItem { key1: 2, key2: 20, value: 200 });

    soteria::assert(
        map.get1(&1u8).map(|i| i.value) == Some(100),
        "key1 round-trips",
    );
    soteria::assert(
        map.get2(&20u8).map(|i| i.value) == Some(200),
        "key2 round-trips",
    );
    soteria::assert(map.get1(&9u8).is_none(), "absent key1 not found");

    soteria::assert(
        map.insert_unique(BiItem { key1: 1, key2: 99, value: 0 }).is_err(),
        "a duplicate key1 is rejected",
    );
    soteria::assert(
        map.insert_unique(BiItem { key1: 99, key2: 10, value: 0 }).is_err(),
        "a duplicate key2 is rejected",
    );

    map.validate(ValidateCompact::Compact).expect("valid bijective map");
}

/// Structural soundness across *arbitrary operation sequences* under an
/// arbitrarily adversarial (lawless) hash.
///
/// This is the proof analog of the model-based `proptest_ops` test.
///
/// # Notes
///
/// We only exercise `remove1` -- `remove2` is symmetric, and we want to
/// keep the proofs fast enough to run in CI.
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
    let mut map: BiHashMap<BiItem, LawlessHasher> =
        BiHashMap::with_hasher(LawlessHasher);

    for _ in 0..SEQ_OPS {
        let op: u8 = soteria::nondet_bytes();
        soteria::assume(op < 2);

        match op {
            0 => {
                let k1 = nondet_u8_below(SEQ_KEYS);
                let k2 = nondet_u8_below(SEQ_KEYS);
                let _ =
                    map.insert_unique(BiItem { key1: k1, key2: k2, value: 0 });
            }
            // op == 1
            _ => {
                let k1 = nondet_u8_below(SEQ_KEYS);
                let _ = map.remove1(&k1);
            }
        }

        map.validate_structural(ValidateCompact::NonCompact).expect(
            "both tables stay sound and in sync after every op under any hash",
        );
    }

    // Drop is quite slow under Soteria and not relevant to soundness today.
    // Since we set `--ignore-leaks`, we can skip it.
    //
    // (Drop might become relevant to soundness in the future, in which case we
    // should cover it here.)
    std::mem::forget(map);
}

#[test]
fn overwrite_fail_fast_is_sound() {
    let mut map: BiHashMap<BiItem, LawlessHasher> =
        BiHashMap::with_hasher(LawlessHasher);
    let k1 = nondet_u8_below(SEQ_KEYS);
    let k2 = nondet_u8_below(SEQ_KEYS);

    let _ = map.insert_unique(BiItem { key1: k1, key2: k2, value: 0 });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = map.insert_overwrite(BiItem { key1: k1, key2: k2, value: 1 });
    }));

    map.validate_structural(ValidateCompact::NonCompact).expect(
        "sound whether insert_overwrite completed or fail-fast panicked",
    );
    std::mem::forget(map);
}
