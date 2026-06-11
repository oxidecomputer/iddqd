//! Pathological adversarial impls of `IdOrdItem` / `IdHashItem` /
//! `BiHashItem` / `TriHashItem`, run under miri (Stacked + Tree Borrows) to
//! probe iddqd's unsafe code for UB.
//!
//! The tests in this file might leave maps in an inconsistent state, but not in
//! a way that should cause UB.

use core::cell::Cell;
use iddqd::{
    BiHashItem, BiHashMap, Comparable, Equivalent, IdHashItem, IdHashMap,
    IdOrdItem, IdOrdMap, TriHashItem, TriHashMap, bi_hash_map, bi_upcast,
    id_hash_map, id_ord_map, id_upcast,
    internal::{ValidateChaos, ValidateCompact},
    tri_upcast,
};
use iddqd_test_utils::{
    panic_safety::{PanickyKey, arm_panic_after, disarm_panic},
    unwind::catch_panic,
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug)]
struct PlainItem {
    id: u32,
}

impl IdHashItem for PlainItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

/// Item using `PanickyKey` so any Hash/Eq/Ord call can be made to panic.
#[derive(Clone, Debug)]
struct PanickyItem {
    id: u32,
}

impl IdHashItem for PanickyItem {
    type Key<'a> = PanickyKey;
    fn key(&self) -> Self::Key<'_> {
        PanickyKey(self.id)
    }
    id_upcast!();
}

impl IdOrdItem for PanickyItem {
    type Key<'a> = PanickyKey;
    fn key(&self) -> Self::Key<'_> {
        PanickyKey(self.id)
    }
    id_upcast!();
}

thread_local! {
    /// When `Some(n)`, the `n`-th subsequent `DropPanicOrdKey::drop` panics.
    /// Counts physical drops rather than `key()` calls, so the trigger is
    /// robust to refactors that create more or fewer keys during an
    /// operation.
    static DROPS_UNTIL_PANIC: Cell<Option<u32>> = const { Cell::new(None) };
}

#[derive(Debug)]
struct DropPanicOrdItem {
    id: u32,
}

#[derive(Debug, Eq)]
struct DropPanicOrdKey {
    id: u32,
}

impl Drop for DropPanicOrdKey {
    fn drop(&mut self) {
        DROPS_UNTIL_PANIC.with(|c| match c.get() {
            None | Some(0) => {}
            Some(1) => {
                // Disarm before panicking so additional drops during
                // unwinding don't double-panic and abort the process.
                c.set(None);
                panic!("DropPanicOrdKey drop panic");
            }
            Some(n) => c.set(Some(n - 1)),
        });
    }
}

impl PartialEq for DropPanicOrdKey {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for DropPanicOrdKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DropPanicOrdKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

struct DropPanicLookup(u32);

impl Equivalent<DropPanicOrdKey> for DropPanicLookup {
    fn equivalent(&self, key: &DropPanicOrdKey) -> bool {
        self.0 == key.id
    }
}

impl Comparable<DropPanicOrdKey> for DropPanicLookup {
    fn compare(&self, key: &DropPanicOrdKey) -> Ordering {
        self.0.cmp(&key.id)
    }
}

impl IdOrdItem for DropPanicOrdItem {
    type Key<'a> = DropPanicOrdKey;

    fn key(&self) -> Self::Key<'_> {
        DropPanicOrdKey { id: self.id }
    }

    id_upcast!();
}

#[test]
fn id_ord_insert_key_drop_panic_leaves_map_valid() {
    let mut map = IdOrdMap::<DropPanicOrdItem>::new();

    // Panic on the *first* key drop after arming. The exact code path exercised
    // depends on the impl, but the broader claim is that a key `Drop` panic at
    // any point during `insert_unique` must leave the map valid.
    DROPS_UNTIL_PANIC.with(|c| c.set(Some(1)));
    let panicked = catch_panic(|| {
        map.insert_unique(DropPanicOrdItem { id: 1 })
            .expect("insert_unique should not return Err here");
    })
    .is_none();
    assert!(panicked, "expected the armed key Drop to panic the insert");

    let validation =
        map.validate(ValidateCompact::NonCompact, ValidateChaos::No);
    assert!(validation.is_ok(), "{validation:?}");
}

#[test]
fn id_ord_insert_key_drop_panic_during_commit_leaves_map_valid() {
    let mut map = IdOrdMap::<DropPanicOrdItem>::new();

    // Skip the first drop (which falls in the duplicate-check window) and panic
    // on the second, which lands between `prepare_insert` and the final commit.
    // This is why IdOrdMap uses two-phase commit.
    //
    // If a future refactor stops creating a second key, this test fails loudly
    // via `assert!(panicked)`. (At that point the test should maybe be
    // deleted.)
    DROPS_UNTIL_PANIC.with(|c| c.set(Some(2)));
    let panicked = catch_panic(|| {
        map.insert_unique(DropPanicOrdItem { id: 1 })
            .expect("insert_unique should not return Err here");
    })
    .is_none();
    assert!(panicked, "expected the armed key Drop to panic the insert");

    let validation =
        map.validate(ValidateCompact::NonCompact, ValidateChaos::No);
    assert!(validation.is_ok(), "{validation:?}");
}

#[test]
fn id_ord_remove_key_drop_panic_leaves_map_valid() {
    let mut map = IdOrdMap::<DropPanicOrdItem>::new();
    map.insert_unique(DropPanicOrdItem { id: 1 })
        .expect("insert_unique on fresh map should succeed");

    // The remove path constructs one key before the prepare/commit window
    // and drops it inside that window, so panicking on the first drop lands
    // the panic in the post-`prepare_remove`, pre-commit drop.
    DROPS_UNTIL_PANIC.with(|c| c.set(Some(1)));
    let panicked = catch_panic(|| {
        let _ = map.remove(&DropPanicLookup(1));
    })
    .is_none();
    assert!(panicked, "expected the armed key Drop to panic the remove");

    let validation =
        map.validate(ValidateCompact::NonCompact, ValidateChaos::No);
    assert!(validation.is_ok(), "{validation:?}");
}

// Test: `OuterItem::key()` calls into a different `IdOrdMap`, so the inner
// `find_index` clobbers the `CMP` thread-local that the outer op was relying
// on. This panics but must not cause UB.

thread_local! {
    static INNER_MAP: RefCell<IdOrdMap<InnerItem>> =
        const { RefCell::new(IdOrdMap::new()) };
    static REENTER: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
struct InnerItem {
    id: u32,
}

impl IdOrdItem for InnerItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

#[derive(Debug)]
struct OuterItem {
    id: u32,
}

impl IdOrdItem for OuterItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        if REENTER.with(Cell::get) {
            INNER_MAP.with(|m| {
                let _ = m.borrow().get(&self.id);
            });
        }
        self.id
    }
    id_upcast!();
}

#[test]
fn cross_map_reentry_does_not_cause_ub() {
    INNER_MAP.with(|m| {
        let mut m = m.borrow_mut();
        for i in 0..8 {
            m.insert_unique(InnerItem { id: i }).unwrap();
        }
    });

    let mut outer = IdOrdMap::<OuterItem>::new();
    for i in 0..8 {
        outer.insert_unique(OuterItem { id: i }).unwrap();
    }

    REENTER.with(|r| r.set(true));
    let _ = catch_panic(|| {
        for i in 0..16 {
            let _ = outer.get(&i);
        }
        for item in outer.iter_mut() {
            let _ = item.id;
        }
        for i in 100..104 {
            let _ = outer.insert_unique(OuterItem { id: i });
        }
    });
    REENTER.with(|r| r.set(false));

    // Use `ValidateChaos::Yes` because CMP-clobbering may have left the BTree
    // in an inconsistent sort order. But there shouldn't be any duplicates.
    let _ = outer.validate(ValidateCompact::NonCompact, ValidateChaos::Yes);
}

// Test: The hash/Eq panic fires inside `make_hash`, after the `&mut T` has been
// obtained via the raw pointer (IdOrdMap) or via `ValuesMut` (IdHashMap).
// Catching the panic and continuing the iterator must not double-borrow.

/// `PanickyKey` keeps panicking once its countdown hits 0 — including in
/// `RefMut::drop`. Drop the yielded item *inside* the closure so its panic
/// is caught, then disarm after the first panic to let iteration finish.
fn drain_with_one_panic<I, T>(mut iter: I) -> usize
where
    I: Iterator<Item = T>,
{
    let mut yielded = 0;
    loop {
        match catch_panic(|| iter.next().is_some()) {
            Some(true) => yielded += 1,
            Some(false) => return yielded,
            None => disarm_panic(),
        }
    }
}

#[test]
fn id_hash_panic_during_iter_mut_no_ub() {
    let mut map = IdHashMap::<PanickyItem>::new();
    for i in 0..16 {
        map.insert_unique(PanickyItem { id: i }).unwrap();
    }
    arm_panic_after(3);
    let count = drain_with_one_panic(map.iter_mut());
    disarm_panic();
    assert!(count >= 15, "drained only {count}");
}

#[test]
fn id_ord_panic_during_iter_mut_no_ub() {
    let mut map = IdOrdMap::<PanickyItem>::new();
    for i in 0..16 {
        map.insert_unique(PanickyItem { id: i }).unwrap();
    }
    arm_panic_after(3);
    let count = drain_with_one_panic(map.iter_mut());
    disarm_panic();
    assert!(count >= 15, "drained only {count}");
}

// Test: key using interior mutability where the returned value changes over
// time.

thread_local! {
    static SHIFTY_KEY: Cell<u32> = const { Cell::new(0) };
}

#[derive(Debug)]
struct ShiftyItem {
    seed: u32,
}

impl IdOrdItem for ShiftyItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.seed ^ SHIFTY_KEY.with(Cell::get)
    }
    id_upcast!();
}

#[test]
fn shifty_key_no_ub() {
    let mut map = IdOrdMap::<ShiftyItem>::new();
    for i in 0..16 {
        let _ = map.insert_unique(ShiftyItem { seed: i });
    }
    for shift in 1..8 {
        SHIFTY_KEY.with(|s| s.set(shift));
        let _ = catch_panic(|| {
            for item in map.iter_mut() {
                let _ = item.seed;
            }
            for k in 0..32 {
                let _ = map.get(&k);
            }
        });
    }
    SHIFTY_KEY.with(|s| s.set(0));
}

// Test: panic in the middle of an IdOrdMap operation, then iter_mut.

#[test]
fn panic_during_ord_op_then_iter_mut_no_dup() {
    let mut map = IdOrdMap::<PanickyItem>::new();
    for i in 0..32 {
        map.insert_unique(PanickyItem { id: i }).unwrap();
    }
    arm_panic_after(3);
    let _ = catch_panic(|| {
        let _ = map.insert_unique(PanickyItem { id: 1000 });
    });
    disarm_panic();

    let mut seen = std::collections::HashSet::new();
    for item in map.iter_mut() {
        assert!(seen.insert(item.id), "iter_mut yielded id={} twice", item.id);
    }
}

// Test: Always return an Eq for IdHashMap.

#[derive(Debug)]
struct LyingEqItem {
    id: u32,
}

#[expect(clippy::derived_hash_with_manual_eq)]
#[derive(Hash)]
struct LyingEqKey {
    id: u32,
}

impl PartialEq for LyingEqKey {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}
impl Eq for LyingEqKey {}

impl IdHashItem for LyingEqItem {
    type Key<'a> = LyingEqKey;
    fn key(&self) -> Self::Key<'_> {
        LyingEqKey { id: self.id }
    }
    id_upcast!();
}

#[test]
fn lying_eq_no_ub() {
    let mut map = IdHashMap::<LyingEqItem>::new();
    for i in 0..16 {
        let _ = map.insert_unique(LyingEqItem { id: i });
        let _ = map.insert_unique(LyingEqItem { id: i });
    }
    let len = map.len();
    let count = map.iter_mut().count();
    assert_eq!(count, len);
}

// Test: pathological Eq on hash keys must not let table-driven mutable paths
// yield overlapping indexes after remove/reinsert.

thread_local! {
    static MISDIRECTED_EQ_MODE: Cell<bool> = const { Cell::new(false) };
}

struct MisdirectedEqKey {
    id: u32,
}

impl Hash for MisdirectedEqKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.id <= 1 {
            0u32.hash(state);
        } else {
            self.id.hash(state);
        }
    }
}

impl PartialEq for MisdirectedEqKey {
    fn eq(&self, other: &Self) -> bool {
        if MISDIRECTED_EQ_MODE.with(Cell::get) {
            match (self.id, other.id) {
                // Under correct behavior, (0, 0) and (1, 1) would return true
                // and (0, 1) and (1, 0) would return false. Invert the result
                // for these IDs.
                (0, 1) | (1, 0) => true,
                (0, 0) | (1, 1) => false,
                _ => self.id == other.id,
            }
        } else {
            self.id == other.id
        }
    }
}
impl Eq for MisdirectedEqKey {}

struct ExactLookupKey {
    id: u32,
}

impl Hash for ExactLookupKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.id <= 1 {
            0u32.hash(state);
        } else {
            self.id.hash(state);
        }
    }
}

impl Equivalent<MisdirectedEqKey> for ExactLookupKey {
    fn equivalent(&self, key: &MisdirectedEqKey) -> bool {
        // This is always correct.
        self.id == key.id
    }
}

#[derive(Debug)]
struct MisdirectedEqIdHashItem {
    id: u32,
}

impl IdHashItem for MisdirectedEqIdHashItem {
    type Key<'a> = MisdirectedEqKey;
    fn key(&self) -> Self::Key<'_> {
        MisdirectedEqKey { id: self.id }
    }
    id_upcast!();
}

#[test]
fn id_hash_misdirected_eq_remove_reinsert_retain_no_aliasing() {
    let mut map = IdHashMap::<MisdirectedEqIdHashItem>::with_capacity(16);
    // Insert two items under correct behavior.
    map.insert_unique(MisdirectedEqIdHashItem { id: 0 }).unwrap();
    map.insert_unique(MisdirectedEqIdHashItem { id: 1 }).unwrap();

    // Now turn on the pathological behavior.
    MISDIRECTED_EQ_MODE.with(|c| c.set(true));
    // The old bug was in the internal table cleanup after that lookup. It knew
    // the `ItemIndex` for id 1, but searched the hash table again using the
    // item's key equality. Under `MisdirectedEqKey`, key 1 matches the table
    // entry for key 0 but not itself, so cleanup could remove key 0's table
    // entry while removing id 1 from `ItemSet`.
    //
    // The fixed code uses the key only to compute the hash, then removes the
    // table entry whose stored `ItemIndex` is exactly the selected index.
    let removed = map.remove(&ExactLookupKey { id: 1 }).unwrap();
    MISDIRECTED_EQ_MODE.with(|c| c.set(false));
    assert_eq!(removed.id, 1);
    map.validate_structural(ValidateCompact::NonCompact)
        .expect("structurally sound after misdirected remove");

    map.insert_unique(MisdirectedEqIdHashItem { id: 2 }).unwrap();

    map.retain(|_| false);
    assert!(map.is_empty());
}

#[derive(Debug)]
struct MisdirectedEqBiHashItem {
    id: u32,
}

impl BiHashItem for MisdirectedEqBiHashItem {
    type K1<'a> = MisdirectedEqKey;
    type K2<'a> = u32;
    fn key1(&self) -> Self::K1<'_> {
        MisdirectedEqKey { id: self.id }
    }
    fn key2(&self) -> Self::K2<'_> {
        self.id + 10
    }
    bi_upcast!();
}

#[test]
fn bi_hash_misdirected_eq_remove_reinsert_retain_no_aliasing() {
    let mut map = BiHashMap::<MisdirectedEqBiHashItem>::with_capacity(16);
    // Insert two items under correct behavior.
    map.insert_unique(MisdirectedEqBiHashItem { id: 0 }).unwrap();
    map.insert_unique(MisdirectedEqBiHashItem { id: 1 }).unwrap();

    // Now turn on the pathological behavior.
    MISDIRECTED_EQ_MODE.with(|c| c.set(true));
    // The old bug was in the internal table cleanup after that lookup. It knew
    // the `ItemIndex` for id 1, but searched the hash table again using the
    // item's key equality. Under `MisdirectedEqKey`, key 1 matches the table
    // entry for key 0 but not itself, so cleanup could remove key 0's table
    // entry while removing id 1 from `ItemSet`.
    //
    // The fixed code uses the key only to compute the hash, then removes the
    // table entry whose stored `ItemIndex` is exactly the selected index.
    let removed = map.remove1(&ExactLookupKey { id: 1 }).unwrap();
    MISDIRECTED_EQ_MODE.with(|c| c.set(false));
    assert_eq!(removed.id, 1);
    map.validate_structural(ValidateCompact::NonCompact)
        .expect("structurally sound after misdirected remove");

    map.insert_unique(MisdirectedEqBiHashItem { id: 2 }).unwrap();

    map.retain(|_| false);
    assert!(map.is_empty());
}

#[derive(Debug)]
struct MisdirectedEqTriHashItem {
    id: u32,
}

impl TriHashItem for MisdirectedEqTriHashItem {
    type K1<'a> = MisdirectedEqKey;
    type K2<'a> = u32;
    type K3<'a> = u32;
    fn key1(&self) -> Self::K1<'_> {
        MisdirectedEqKey { id: self.id }
    }
    fn key2(&self) -> Self::K2<'_> {
        self.id + 10
    }
    fn key3(&self) -> Self::K3<'_> {
        self.id + 20
    }
    tri_upcast!();
}

#[test]
fn tri_hash_misdirected_eq_remove_reinsert_retain_no_aliasing() {
    let mut map = TriHashMap::<MisdirectedEqTriHashItem>::with_capacity(16);
    // Insert two items under correct behavior.
    map.insert_unique(MisdirectedEqTriHashItem { id: 0 }).unwrap();
    map.insert_unique(MisdirectedEqTriHashItem { id: 1 }).unwrap();

    // Now turn on the pathological behavior.
    MISDIRECTED_EQ_MODE.with(|c| c.set(true));
    // The old bug was in the internal table cleanup after that lookup. It knew
    // the `ItemIndex` for id 1, but searched the hash table again using the
    // item's key equality. Under `MisdirectedEqKey`, key 1 matches the table
    // entry for key 0 but not itself, so cleanup could remove key 0's table
    // entry while removing id 1 from `ItemSet`.
    //
    // The fixed code uses the key only to compute the hash, then removes the
    // table entry whose stored `ItemIndex` is exactly the selected index.
    let removed = map.remove1(&ExactLookupKey { id: 1 }).unwrap();
    MISDIRECTED_EQ_MODE.with(|c| c.set(false));
    assert_eq!(removed.id, 1);
    map.validate_structural(ValidateCompact::NonCompact)
        .expect("structurally sound after misdirected remove");

    map.insert_unique(MisdirectedEqTriHashItem { id: 2 }).unwrap();

    map.retain(|_| false);
    assert!(map.is_empty());
}

#[derive(Debug)]
struct AlwaysEqItem;

struct AlwaysEqKey;

// Constant hash + always-equal: every probe hits the first occupied slot.
impl Hash for AlwaysEqKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u64.hash(state);
    }
}
impl PartialEq for AlwaysEqKey {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for AlwaysEqKey {}

impl IdHashItem for AlwaysEqItem {
    type Key<'a> = AlwaysEqKey;
    fn key(&self) -> Self::Key<'_> {
        AlwaysEqKey
    }
    id_upcast!();
}

#[test]
fn always_eq_no_ub() {
    let mut map = IdHashMap::<AlwaysEqItem>::new();
    map.insert_unique(AlwaysEqItem).unwrap();
    for _ in 0..8 {
        assert!(map.insert_unique(AlwaysEqItem).is_err());
    }
    assert_eq!(map.len(), 1);
    assert_eq!(map.iter_mut().count(), 1);
}

// Test: Ord that always returns Less.
//
// A user `Ord` that returns a constant ordering corrupts the BTree's sort
// invariant. iter_mut must still never yield the same index twice.

thread_local! {
    static LIE_ORD: Cell<Option<Ordering>> = const { Cell::new(None) };
}

#[derive(Debug)]
struct LyingOrdItem {
    id: u32,
    value: u32,
}

#[derive(PartialEq, Eq)]
struct LyingOrdKey {
    id: u32,
}

impl Hash for LyingOrdKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialOrd for LyingOrdKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LyingOrdKey {
    fn cmp(&self, other: &Self) -> Ordering {
        LIE_ORD.with(Cell::get).unwrap_or_else(|| self.id.cmp(&other.id))
    }
}

impl IdOrdItem for LyingOrdItem {
    type Key<'a> = LyingOrdKey;
    fn key(&self) -> Self::Key<'_> {
        LyingOrdKey { id: self.id }
    }
    id_upcast!();
}

#[derive(Debug)]
struct HashBlindOrdItem {
    id: u32,
    value: u32,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct HashBlindOrdKey {
    id: u32,
}

impl Hash for HashBlindOrdKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // `IdOrdMap::RefMut` detects key changes by hashing the key before and
        // after the mutable borrow. A user key can defeat that check by
        // colliding all key hashes.
        0u32.hash(state);
    }
}

impl IdOrdItem for HashBlindOrdItem {
    type Key<'a> = HashBlindOrdKey;
    fn key(&self) -> Self::Key<'_> {
        HashBlindOrdKey { id: self.id }
    }
    id_upcast!();
}

#[test]
fn id_ord_hash_blind_key_change_remove_reinsert_iter_mut_no_aliasing() {
    let mut map = IdOrdMap::<HashBlindOrdItem>::new();
    for i in 0..64 {
        map.insert_unique(HashBlindOrdItem { id: i, value: 0 }).unwrap();
    }

    // This changes the key's ordering position without changing its hash, so
    // `RefMut` drop does not catch it. The B-tree is now logically misordered.
    {
        let mut item = map.get_mut(&HashBlindOrdKey { id: 0 }).unwrap();
        item.id = 10_000;
    }

    // `pop_first` sees the structurally first index. Removal must clear that
    // exact index from the B-tree even though comparator-based search would be
    // confused by the changed key.
    let removed = map.pop_first().unwrap();
    assert_eq!(removed.id, 10_000);

    // The item set reuses the removed slot. If the old B-tree entry was left
    // behind, iter_mut would yield two RefMuts to this same slot.
    map.insert_unique(HashBlindOrdItem { id: 20_000, value: 0 }).unwrap();

    let mut items: Vec<_> = map.iter_mut().collect();
    for item in &mut items {
        item.value += 1;
    }
}

// Test: hash-map analog of the IdOrdMap test above.

#[derive(Debug)]
struct ForgettableHashItem {
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ForgettableHashKey(u32);

impl IdHashItem for ForgettableHashItem {
    type Key<'a> = ForgettableHashKey;
    fn key(&self) -> Self::Key<'_> {
        ForgettableHashKey(self.id)
    }
    id_upcast!();
}

#[derive(Debug)]
struct ForgettableBiHashItem {
    id: u32,
    alt: u32,
}

impl BiHashItem for ForgettableBiHashItem {
    type K1<'a> = ForgettableHashKey;
    type K2<'a> = ForgettableHashKey;
    fn key1(&self) -> Self::K1<'_> {
        ForgettableHashKey(self.id)
    }
    fn key2(&self) -> Self::K2<'_> {
        ForgettableHashKey(self.alt)
    }
    bi_upcast!();
}

#[derive(Debug)]
struct ForgettableTriHashItem {
    id: u32,
    alt: u32,
    third: u32,
}

impl TriHashItem for ForgettableTriHashItem {
    type K1<'a> = ForgettableHashKey;
    type K2<'a> = ForgettableHashKey;
    type K3<'a> = ForgettableHashKey;
    fn key1(&self) -> Self::K1<'_> {
        ForgettableHashKey(self.id)
    }
    fn key2(&self) -> Self::K2<'_> {
        ForgettableHashKey(self.alt)
    }
    fn key3(&self) -> Self::K3<'_> {
        ForgettableHashKey(self.third)
    }
    tri_upcast!();
}

#[test]
fn id_hash_silent_key_change_entry_remove_no_panic() {
    let mut map = IdHashMap::<ForgettableHashItem>::new();
    for id in 0..8u32 {
        map.insert_unique(ForgettableHashItem { id }).unwrap();
    }

    let id_hash_map::Entry::Occupied(mut entry) =
        map.entry(ForgettableHashKey(3))
    else {
        panic!("we just inserted id 3 in the loop above");
    };
    let mut ref_mut = entry.get_mut();
    ref_mut.id = 10_000;
    // This bypasses the drop-time hash equality check on the ID.
    std::mem::forget(ref_mut);

    // Now call `entry.remove()`. This will not succeed at efficient removal.
    // But we have a linear-scan fallback in place, due to which cleanup
    // succeeds.
    let removed = entry.remove();
    assert_eq!(removed.id, 10_000);

    // The seven other items must still be findable.
    for id in [0u32, 1, 2, 4, 5, 6, 7] {
        assert!(map.contains_key(&ForgettableHashKey(id)));
    }
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation remove");
}

#[test]
fn bi_hash_silent_secondary_key_change_retain_no_panic() {
    let mut map = BiHashMap::<ForgettableBiHashItem>::new();
    for id in 0..8u32 {
        map.insert_unique(ForgettableBiHashItem { id, alt: id + 100 }).unwrap();
    }

    let mut ref_mut = map.get1_mut(&ForgettableHashKey(3)).unwrap();
    ref_mut.alt = 10_000;
    // This bypasses the drop-time hash equality check on key2, leaving the k2
    // table entry in the old hash bucket.
    std::mem::forget(ref_mut);
    map.validate_structural(ValidateCompact::NonCompact)
        .expect("structurally sound with a stranded key2 entry");

    map.retain(|_| false);

    assert!(map.is_empty());
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation retain");
}

#[test]
fn tri_hash_silent_secondary_key_change_retain_no_panic() {
    let mut map = TriHashMap::<ForgettableTriHashItem>::new();
    for id in 0..8u32 {
        map.insert_unique(ForgettableTriHashItem {
            id,
            alt: id + 100,
            third: id + 200,
        })
        .unwrap();
    }

    let mut ref_mut = map.get1_mut(&ForgettableHashKey(3)).unwrap();
    ref_mut.third = 10_000;
    // This bypasses the drop-time hash equality check on key3, leaving the k3
    // table entry in the old hash bucket.
    std::mem::forget(ref_mut);
    map.validate_structural(ValidateCompact::NonCompact)
        .expect("structurally sound with a stranded key3 entry");

    map.retain(|_| false);

    assert!(map.is_empty());
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation retain");
}

// Tests the scenario where:
//
// * A silent key change strands that key's table entry in its old hash bucket.
// * A later `insert_overwrite` finds the item through a different, unchanged key.
//
// This must still clean up the stranded entry by doing a linear scan.
#[test]
fn bi_hash_silent_secondary_key_change_insert_overwrite() {
    let mut map: BiHashMap<ForgettableBiHashItem, _> =
        BiHashMap::with_capacity_and_hasher(
            8,
            foldhash::fast::FixedState::with_seed(0),
        );
    for id in 0..8u32 {
        map.insert_unique(ForgettableBiHashItem { id, alt: id + 100 }).unwrap();
    }

    // Move id=3's key2 from 103 to 10_000, stranding the k2 entry in hash(103).
    let mut ref_mut = map.get1_mut(&ForgettableHashKey(3)).unwrap();
    ref_mut.alt = 10_000;
    // RefMut would panic if its Drop were allowed to run. Forget it to avoid the panic.
    std::mem::forget(ref_mut);
    // This lookup fails -- this acts as evidence that while cleaning up the
    // stranded entry, we will in fact do a linear scan.
    assert!(
        map.get2(&ForgettableHashKey(10_000)).is_none(),
        "key2=10_000 must be unreachable by its new hash"
    );

    // Now try overwriting with a different value.
    let dups =
        map.insert_overwrite(ForgettableBiHashItem { id: 3, alt: 9_999 });
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].alt, 10_000);

    for id in [0u32, 1, 2, 4, 5, 6, 7] {
        assert!(map.contains_key1(&ForgettableHashKey(id)));
    }
    assert_eq!(map.get1(&ForgettableHashKey(3)).unwrap().alt, 9_999);
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation insert_overwrite");
}

// Same test as above but using the Entry API.
#[test]
fn bi_hash_silent_secondary_key_change_entry_remove() {
    let mut map: BiHashMap<ForgettableBiHashItem, _> =
        BiHashMap::with_capacity_and_hasher(
            8,
            foldhash::fast::FixedState::with_seed(0),
        );
    for id in 0..8u32 {
        map.insert_unique(ForgettableBiHashItem { id, alt: id + 100 }).unwrap();
    }

    let mut ref_mut = map.get1_mut(&ForgettableHashKey(3)).unwrap();
    ref_mut.alt = 10_000;
    std::mem::forget(ref_mut);
    assert!(
        map.get2(&ForgettableHashKey(10_000)).is_none(),
        "key2=10_000 must be unreachable by its new hash"
    );

    let bi_hash_map::Entry::Occupied(entry) =
        map.entry(ForgettableHashKey(3), ForgettableHashKey(9_999))
    else {
        panic!("key1=3 is present, so the entry is occupied");
    };
    let removed = entry.remove();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].alt, 10_000);

    assert!(!map.contains_key1(&ForgettableHashKey(3)));
    for id in [0u32, 1, 2, 4, 5, 6, 7] {
        assert!(map.contains_key1(&ForgettableHashKey(id)));
    }
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation entry remove");
}

// Same test as above but using `TriHashMap`.
#[test]
fn tri_hash_silent_tertiary_key_change_insert_overwrite() {
    let mut map: TriHashMap<ForgettableTriHashItem, _> =
        TriHashMap::with_capacity_and_hasher(
            8,
            foldhash::fast::FixedState::with_seed(0),
        );
    for id in 0..8u32 {
        map.insert_unique(ForgettableTriHashItem {
            id,
            alt: id + 100,
            third: id + 200,
        })
        .unwrap();
    }

    let mut ref_mut = map.get1_mut(&ForgettableHashKey(3)).unwrap();
    ref_mut.third = 10_000;
    std::mem::forget(ref_mut);
    assert!(
        map.get3(&ForgettableHashKey(10_000)).is_none(),
        "key3=10_000 must be unreachable by its new hash"
    );

    let dups = map.insert_overwrite(ForgettableTriHashItem {
        id: 3,
        alt: 999,
        third: 888,
    });
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].third, 10_000);

    for id in [0u32, 1, 2, 4, 5, 6, 7] {
        assert!(map.contains_key1(&ForgettableHashKey(id)));
    }
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after silent-mutation insert_overwrite");
}

#[test]
fn lying_ord_remove_must_not_remove_wrong_btree_entry() {
    // Build a 64-element map under an honest `Ord`. The B-tree is sorted by
    // user key (id); ids and ItemIndex values coincide here, so the tree's
    // structural order also matches index order, and Index{0} is the leftmost
    // leaf.
    let mut map = IdOrdMap::<LyingOrdItem>::new();
    for i in 0..64 {
        map.insert_unique(LyingOrdItem { id: i, value: 0 }).unwrap();
    }

    // `entry()` walks the tree under the honest comparator and resolves id=0
    // to its exact ItemIndex (0). The OccupiedEntry remembers that index but
    // releases the &mut, so subsequent operations re-descend the tree.
    let entry = map.entry(LyingOrdKey { id: 0 });
    let removed = match entry {
        id_ord_map::Entry::Occupied(entry) => {
            // Arm the lying comparator *after* `entry()` resolved the index
            // but *before* `Entry::remove` descends the tree again.
            //
            // `Entry::remove` routes through `remove_by_index(0)`, which
            // calls `prepare_remove(0, &key, lookup)`. That re-enters
            // `BTreeMap::entry(Index::new(0))` under `insert_cmp`. This
            // second descent is the dangerous one.
            //
            // Stepping through a comparator without tie breaks (the bug):
            //
            // 1. At the root, BTreeMap compares search index 0 against
            //    some stored index, say X. `insert_cmp` lands in the
            //    `a == index` arm and returns `key.compare(&lookup(X))`.
            // 2. LIE_ORD=Equal makes that `Ordering::Equal`.
            // 3. BTreeMap concludes the keys match and reports
            //    `Occupied(index X)` -- some unrelated index near the
            //    root, never 0.
            // 4. `entry.remove_entry()` deletes the node for X.
            // 5. The tree entry for index 0 is left behind as an orphan,
            //    and a valid live entry for X is now unlinked.
            // 6. Back in `remove_by_index`, `self.items.remove(0)` frees
            //    physical slot 0 from the item set. The orphan now points
            //    at a freed slot, and slot X is still occupied but
            //    unreachable through the tree.
            //
            // The tiebreaker comparator fixes step 1-3. Within `insert_cmp`, we
            // now say `key.compare(...).then_with(|| index.cmp(&b))`.
            //
            // * Under LIE_ORD=Equal the primary collapses to Equal, but the
            //   tiebreaker comparator resolves to `0.cmp(&b)`, which is Less
            //   for every b != 0.
            // * As a result, the BTreeMap walks consistently left, reaches
            //   the leaf holding index 0 (where the top-of-comparator `a == b`
            //   short-circuit fires), and reports Occupied(0).
            // * If the tree's structure didn't align with index order and the
            //   walk passed index 0 by, the descent would land at Vacant. That
            //   is still okay because `prepare_remove` would then return Missing
            //   and `remove_by_index` would fall back to the linear `remove_exact`.
            //
            // Either path removes the entry for index 0, and only that one.
            LIE_ORD.with(|c| c.set(Some(Ordering::Equal)));
            entry.remove()
        }
        id_ord_map::Entry::Vacant(_) => panic!("id 0 should be present"),
    };
    assert_eq!(removed.id, 0);

    // Reinsert under a different lie. We use `Greater` to make
    // `insert_unique`'s duplicate-detection `find_index` walk the tree under
    // the lying comparator too. This directs the walk consistently right and
    // lets the walk reach Vacant without seeing a false match.
    //
    // ItemSet's free chain is LIFO, so the new item is assigned exactly the
    // ItemIndex that we just freed above, 0.
    //
    // Under the un-tiebroken (old) comparator, this is what triggers the UB:
    //
    // 1. The orphan tree entry for index 0 is still leftmost in the tree.
    // 2. `prepare_insert(0, ...)` walks under LIE_ORD=Greater. Every
    //    comparison takes `insert_cmp`'s `a == index` arm and returns
    //    `Greater`, so the walk heads right.
    // 3. The walk never visits the leftmost orphan, never sees Equal, and
    //    terminates at Vacant on the right end. A *second* tree node for
    //    index 0 is inserted there.
    // 4. The B-tree now holds two distinct nodes whose `Index::value()`
    //    both equal 0. As a result, the index uniqueness invariant is
    //    violated.
    // 5. `iter_mut` walks the tree in B-tree order and yields a
    //    `RefMut` for slot 0 twice, so that there are two mutable
    //    aliases to the same physical item.
    //
    // Under the tiebreaker comparator, there is no orphaned index (the remove
    // above hit the right entry), so `prepare_insert` produces exactly one tree
    // node for the reused index, and `iter_mut` yields each item once.
    LIE_ORD.with(|c| c.set(Some(Ordering::Greater)));
    map.insert_unique(LyingOrdItem { id: 10_000, value: 0 }).unwrap();
    LIE_ORD.with(|c| c.set(None));

    // Walk through all the items via `iter_mut` so any &mut aliasing is
    // detected by Miri.
    let mut items: Vec<_> = map.iter_mut().collect();
    for item in &mut items {
        item.value += 1;
    }
}

#[test]
fn lying_ord_iter_mut_no_duplicate_yield() {
    let mut map = IdOrdMap::<LyingOrdItem>::new();
    for i in 0..16 {
        map.insert_unique(LyingOrdItem { id: i, value: 0 }).unwrap();
    }
    LIE_ORD.with(|c| c.set(Some(Ordering::Less)));
    let _ = catch_panic(|| {
        for i in 100..110 {
            let _ = map.insert_unique(LyingOrdItem { id: i, value: 0 });
        }
    });

    let mut seen = std::collections::HashSet::new();
    for item in map.iter_mut() {
        let _ = item.value;
        assert!(seen.insert(item.id), "iter_mut yielded id={} twice", item.id);
    }
    LIE_ORD.with(|c| c.set(None));
}

#[test]
fn lying_ord_remove_reinsert_iter_mut_no_aliasing() {
    let mut map = IdOrdMap::<LyingOrdItem>::new();
    // Insert a bunch of items.
    for i in 0..64 {
        map.insert_unique(LyingOrdItem { id: i, value: 0 }).unwrap();
    }

    // Look up item 0 while the `Ord` is correct...
    let entry = map.entry(LyingOrdKey { id: 0 });
    let removed = match entry {
        id_ord_map::Entry::Occupied(entry) => {
            // Now switch the Ord implementation to always return
            // `Ordering::Less`.
            LIE_ORD.with(|c| c.set(Some(Ordering::Less)));
            // Removal must not be confused by the pathological comparator: it
            // has already selected the exact index being removed.
            entry.remove()
        }
        id_ord_map::Entry::Vacant(_) => panic!("id 0 should be present"),
    };
    assert_eq!(removed.id, 0);

    // Now insert an item with a comparator which always returns
    // `Ordering::Greater`. The item set will allocate index 0 to the new item
    // because it uses LRU semantics for the free chain. If the previous index 0
    // was left behind in the B-tree table, iter_mut would hand out two mutable
    // aliases to the item we just inserted at index 0.
    LIE_ORD.with(|c| c.set(Some(Ordering::Greater)));
    map.insert_unique(LyingOrdItem { id: 10_000, value: 0 }).unwrap();
    LIE_ORD.with(|c| c.set(None));

    let mut items: Vec<_> = map.iter_mut().collect();
    for item in &mut items {
        item.value += 1;
    }
}

// Test: Drop panic during remove.

thread_local! {
    static DROP_PANIC_ARMED: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
struct DropPanicItem {
    id: u32,
    armed: bool,
}

impl Drop for DropPanicItem {
    fn drop(&mut self) {
        if self.armed && DROP_PANIC_ARMED.with(Cell::get) {
            DROP_PANIC_ARMED.with(|c| c.set(false));
            panic!("DropPanicItem::drop on id {}", self.id);
        }
    }
}

impl IdHashItem for DropPanicItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

#[test]
fn drop_panic_during_remove_no_ub() {
    let mut map = IdHashMap::<DropPanicItem>::new();
    for i in 0..8 {
        map.insert_unique(DropPanicItem { id: i, armed: false }).unwrap();
    }
    let _ = map.insert_overwrite(DropPanicItem { id: 5, armed: true });

    DROP_PANIC_ARMED.with(|c| c.set(true));
    let _ = catch_panic(|| {
        let _ = map.remove(&5);
    });
    DROP_PANIC_ARMED.with(|c| c.set(false));

    assert_eq!(map.iter_mut().count(), 7);
    assert!(map.get(&5).is_none());
}

#[test]
fn drop_panic_during_retain_leaves_map_valid() {
    let mut map = IdHashMap::<DropPanicItem>::new();
    for i in 0..8 {
        map.insert_unique(DropPanicItem { id: i, armed: i == 5 }).unwrap();
    }

    DROP_PANIC_ARMED.with(|c| c.set(true));
    let result = catch_panic(std::panic::AssertUnwindSafe(|| {
        map.retain(|item| item.id != 5);
    }));
    DROP_PANIC_ARMED.with(|c| c.set(false));

    assert!(result.is_none(), "expected retained-away item drop to panic");
    assert_eq!(map.iter_mut().count(), 7);
    assert!(map.get(&5).is_none());
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after retain drop panic");
}

#[test]
fn drop_panic_during_clear_leaves_map_valid() {
    let mut map = IdHashMap::<DropPanicItem>::new();
    for i in 0..8 {
        map.insert_unique(DropPanicItem { id: i, armed: i == 5 }).unwrap();
    }

    DROP_PANIC_ARMED.with(|c| c.set(true));
    let result = catch_panic(std::panic::AssertUnwindSafe(|| {
        map.clear();
    }));
    DROP_PANIC_ARMED.with(|c| c.set(false));

    assert!(result.is_none(), "expected item drop during clear to panic");
    assert!(map.is_empty());
    assert!(map.get(&5).is_none());
    map.validate(ValidateCompact::NonCompact)
        .expect("map remains valid after clear drop panic");
}

// Test: retain callback panic.

#[test]
fn retain_callback_panic_no_ub() {
    let mut map = IdHashMap::<PlainItem>::new();
    for i in 0..16 {
        map.insert_unique(PlainItem { id: i }).unwrap();
    }
    let _ = catch_panic(std::panic::AssertUnwindSafe(|| {
        let mut count = 0;
        map.retain(|_| {
            count += 1;
            if count == 5 {
                panic!("retain callback panic");
            }
            true
        });
    }));
    assert!(map.iter_mut().count() >= 1);
}

// Test: pop_first / pop_last on a chaotically-ordered BTree.

#[test]
fn pop_after_chaos_no_ub() {
    let mut map = IdOrdMap::<LyingOrdItem>::new();
    for i in 0..16 {
        map.insert_unique(LyingOrdItem { id: i, value: 0 }).unwrap();
    }
    LIE_ORD.with(|c| c.set(Some(Ordering::Greater)));
    while catch_panic(|| map.pop_first()).flatten().is_some() {}
    while catch_panic(|| map.pop_last()).flatten().is_some() {}
    LIE_ORD.with(|c| c.set(None));
}

#[cfg(feature = "allocator-api2")]
mod allocator_tests {
    use super::*;
    use allocator_api2::alloc::Global;
    use iddqd_test_utils::{
        alloc_failure::{FailingAlloc, with_failing_alloc},
        panic_safety::{PanickyAlloc, arm_panic_after, disarm_panic},
    };
    use std::{collections::BTreeSet, panic::AssertUnwindSafe};

    // An allocator panic during shrink_to_fit must leave the tables and the
    // item set in sync.
    //
    // Without the fix, the inner `Vec::shrink_to_fit` panics after
    // `ItemSet::compact` has already reorganized the slot buffer, but before
    // the outer code can remap the indexes stored in the tables. As a result,
    // `get(key)` ends up reading the wrong physical slot.
    #[test]
    fn shrink_to_fit_panic_keeps_tables_and_items_in_sync() {
        let mut map: IdHashMap<_, _, PanickyAlloc<Global>> =
            IdHashMap::with_capacity_and_hasher_in(
                8,
                foldhash::fast::FixedState::with_seed(0),
                PanickyAlloc::default(),
            );

        // Insert 8 items, then remove two from the middle so `compact` has
        // holes to fill and the `IndexRemap` is non-trivial.
        for id in 0..8u32 {
            map.insert_unique(ForgettableHashItem { id }).unwrap();
        }
        map.remove(&ForgettableHashKey(2)).expect("id 2 was inserted");
        map.remove(&ForgettableHashKey(5)).expect("id 5 was inserted");

        let expected: BTreeSet<u32> = map.iter().map(|item| item.id).collect();

        // Arm the allocator and call `shrink_to_fit`. The first allocate
        // inside `Vec::shrink_to_fit` panics, which is exactly the panic
        // window the fix needs to handle.
        //
        // `ForgettableHashItem` and `ForgettableHashKey` don't invoke the
        // panic countdown from `Hash`/`Eq`/`Drop`, so the count is driven
        // entirely by `PanickyAlloc::allocate` here.
        arm_panic_after(0);
        let result = catch_panic(AssertUnwindSafe(|| map.shrink_to_fit()));
        disarm_panic();
        assert!(
            result.is_none(),
            "shrink_to_fit should have panicked once the allocator was armed"
        );

        // The item set is what `iter()` walks, so it still yields the same
        // logical set of keys.
        let yielded: BTreeSet<u32> = map.iter().map(|item| item.id).collect();
        assert_eq!(
            yielded, expected,
            "iter() should return the same items across the panic"
        );

        // The real test: every item yielded by `iter()` must also be
        // findable by its key via `get()`. With the bug, the tables hold
        // pre-compact indexes that point at either the wrong slot (returns
        // the wrong item) or an out-of-bounds slot (returns None).
        for id in &yielded {
            let got = map.get(&ForgettableHashKey(*id));
            assert_eq!(
                got.map(|item| item.id),
                Some(*id),
                "get({id}) returns the right item"
            );
        }

        // Structural validity should also hold. The panic fires *after*
        // `compact()` returned, so even though the Vec capacity shrink
        // never completed, the items themselves are fully compact and the
        // tables have already been remapped to match.
        map.validate(ValidateCompact::Compact).expect(
            "map should be structurally valid (and compact) after the \
             shrink_to_fit panic",
        );
    }

    // Ensures that when a brand-new key forces the item set to grow,
    // `insert_overwrite` reserves that capacity up front, so a reservation
    // failure leaves the map unchanged.
    //
    // The reserve-before-remove ordering on the duplicate path is covered by
    // the `PanickyAlloc` panic-safety proptest.
    #[test]
    fn bi_insert_overwrite_atomic_on_alloc_failure() {
        let mut map: BiHashMap<_, _, FailingAlloc<Global>> =
            BiHashMap::with_hasher_in(
                foldhash::fast::FixedState::with_seed(0),
                FailingAlloc(Global),
            );
        for id in 0..4u32 {
            map.insert_unique(ForgettableBiHashItem { id, alt: id + 100 })
                .unwrap();
        }
        // Compact the item set so a brand-new key forces a fresh allocation.
        map.shrink_to_fit();

        let before: BTreeSet<_> =
            map.iter().map(|item| (item.id, item.alt)).collect();

        let result = catch_panic(AssertUnwindSafe(|| {
            with_failing_alloc(|| {
                map.insert_overwrite(ForgettableBiHashItem { id: 99, alt: 999 })
            })
        }));
        assert!(
            result.is_none(),
            "insert_overwrite should panic when the reservation fails"
        );

        let after: BTreeSet<_> =
            map.iter().map(|item| (item.id, item.alt)).collect();
        assert_eq!(
            after, before,
            "map must be unchanged after a failed reserve"
        );
        map.validate(ValidateCompact::Compact)
            .expect("map remains valid and compact after a failed reservation");
    }

    #[test]
    fn tri_insert_overwrite_atomic_on_alloc_failure() {
        let mut map: TriHashMap<_, _, FailingAlloc<Global>> =
            TriHashMap::with_hasher_in(
                foldhash::fast::FixedState::with_seed(0),
                FailingAlloc(Global),
            );
        for id in 0..4u32 {
            map.insert_unique(ForgettableTriHashItem {
                id,
                alt: id + 100,
                third: id + 200,
            })
            .unwrap();
        }
        map.shrink_to_fit();

        let before: BTreeSet<_> =
            map.iter().map(|item| (item.id, item.alt, item.third)).collect();

        let result = catch_panic(AssertUnwindSafe(|| {
            with_failing_alloc(|| {
                map.insert_overwrite(ForgettableTriHashItem {
                    id: 99,
                    alt: 999,
                    third: 888,
                })
            })
        }));
        assert!(
            result.is_none(),
            "insert_overwrite should panic when the reservation fails"
        );

        let after: BTreeSet<_> =
            map.iter().map(|item| (item.id, item.alt, item.third)).collect();
        assert_eq!(
            after, before,
            "map must be unchanged after a failed reserve"
        );
        map.validate(ValidateCompact::Compact)
            .expect("map remains valid and compact after a failed reservation");
    }
}
