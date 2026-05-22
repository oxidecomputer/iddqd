//! Pathological adversarial impls of `IdOrdItem` / `IdHashItem`, run under
//! miri (Stacked + Tree Borrows) to probe iddqd's unsafe code for UB.
//!
//! The tests in this file might leave maps in an inconsistent state, but not in
//! a way that should cause UB.

use crate::panic_safety::{PanickyKey, arm_panic_after, disarm_panic};
use core::cell::Cell;
use iddqd::{
    BiHashItem, BiHashMap, Comparable, Equivalent, IdHashItem, IdHashMap,
    IdOrdItem, IdOrdMap, TriHashItem, TriHashMap, bi_upcast, id_ord_map,
    id_upcast,
    internal::{ValidateChaos, ValidateCompact},
    tri_upcast,
};
use iddqd_test_utils::unwind::catch_panic;
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
    static DROP_PANIC_KEY_CALLS: Cell<u32> = const { Cell::new(0) };
    static DROP_PANIC_ON_KEY_DROP: Cell<Option<u32>> = const { Cell::new(None) };
}

#[derive(Debug)]
struct DropPanicOrdItem {
    id: u32,
}

#[derive(Debug, Eq)]
struct DropPanicOrdKey {
    id: u32,
    key_call: u32,
}

impl DropPanicOrdKey {
    fn new(id: u32) -> Self {
        let key_call = DROP_PANIC_KEY_CALLS.with(|c| {
            let next = c.get() + 1;
            c.set(next);
            next
        });
        Self { id, key_call }
    }
}

impl Drop for DropPanicOrdKey {
    fn drop(&mut self) {
        DROP_PANIC_ON_KEY_DROP.with(|c| {
            if c.get() == Some(self.key_call) {
                panic!(
                    "DropPanicOrdKey drop panic on key call {}",
                    self.key_call
                );
            }
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
        DropPanicOrdKey::new(self.id)
    }

    id_upcast!();
}

fn reset_drop_panic_ord_key() {
    DROP_PANIC_KEY_CALLS.with(|c| c.set(0));
    DROP_PANIC_ON_KEY_DROP.with(|c| c.set(None));
}

#[test]
fn id_ord_insert_key_drop_panic_leaves_map_valid() {
    let mut map = IdOrdMap::<DropPanicOrdItem>::new();

    reset_drop_panic_ord_key();
    DROP_PANIC_ON_KEY_DROP.with(|c| c.set(Some(2)));
    let panicked = catch_panic(|| {
        map.insert_unique(DropPanicOrdItem { id: 1 }).unwrap();
    })
    .is_none();
    DROP_PANIC_ON_KEY_DROP.with(|c| c.set(None));
    assert!(panicked);

    let validation =
        map.validate(ValidateCompact::NonCompact, ValidateChaos::No);
    assert!(validation.is_ok(), "{validation:?}");
}

#[test]
fn id_ord_remove_key_drop_panic_leaves_map_valid() {
    let mut map = IdOrdMap::<DropPanicOrdItem>::new();
    map.insert_unique(DropPanicOrdItem { id: 1 }).unwrap();

    reset_drop_panic_ord_key();
    DROP_PANIC_ON_KEY_DROP.with(|c| c.set(Some(2)));
    let panicked = catch_panic(|| {
        let _ = map.remove(&DropPanicLookup(1));
    })
    .is_none();
    DROP_PANIC_ON_KEY_DROP.with(|c| c.set(None));
    assert!(panicked);

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
