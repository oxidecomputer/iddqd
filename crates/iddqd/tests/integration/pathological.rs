//! Pathological adversarial impls of `IdOrdItem` / `IdHashItem` /
//! `BiHashItem`, run under miri (Stacked + Tree Borrows) to probe iddqd's
//! unsafe code for UB.
//!
//! The tests in this file might leave maps in an inconsistent state, but not in
//! a way that should cause UB.

use crate::panic_safety::{PanickyKey, arm_panic_after, disarm_panic};
use core::cell::Cell;
use iddqd::{
    BiHashItem, BiHashMap, IdHashItem, IdHashMap, IdOrdItem, IdOrdMap,
    bi_upcast, id_upcast,
    internal::{ValidateChaos, ValidateCompact},
};
use iddqd_test_utils::unwind::catch_panic;
use std::{
    cell::RefCell,
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug)]
struct PayloadItem {
    id: u32,
    payload: u64,
}

impl IdHashItem for PayloadItem {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

impl IdOrdItem for PayloadItem {
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

// Test: `OuterItem::key()` calls into a different `IdOrdMap`, so the inner
// `find_index` clobbers the `CMP` thread-local that the outer op was relying
// on. This panics but must not cause UB.

thread_local! {
    static INNER_MAP: RefCell<IdOrdMap<InnerItem>> =
        RefCell::new(IdOrdMap::new());
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

// Test: long-lived RefMut + iter_mut.

#[test]
fn id_ord_iter_mut_payload_writes_no_ub() {
    let mut map = IdOrdMap::<PayloadItem>::new();
    for i in 0..32 {
        map.insert_unique(PayloadItem { id: i, payload: 0 }).unwrap();
    }

    let refs: Vec<_> = map.iter_mut().collect();
    for mut r in refs {
        r.payload = (r.id as u64) ^ 0xdead;
    }

    for i in 0..32 {
        assert_eq!(map.get(&i).unwrap().payload, (i as u64) ^ 0xdead);
    }
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

// Test: BiHashMap entry NonUnique → get_disjoint_mut.

#[derive(Debug)]
struct BiItem {
    k1: u32,
    k2: u32,
    payload: u64,
}

impl BiHashItem for BiItem {
    type K1<'a> = u32;
    type K2<'a> = u32;
    fn key1(&self) -> Self::K1<'_> {
        self.k1
    }
    fn key2(&self) -> Self::K2<'_> {
        self.k2
    }
    bi_upcast!();
}

#[test]
fn bi_entry_nonunique_writes_no_ub() {
    let mut map = BiHashMap::<BiItem>::new();
    for i in 0..8u32 {
        map.insert_unique(BiItem { k1: i, k2: 1000 + i, payload: 0 }).unwrap();
    }

    // k1 from item 3, k2 from item 5 → NonUnique → Key12 → get_disjoint_mut.
    let occupied = match map.entry(3, 1005) {
        iddqd::bi_hash_map::Entry::Occupied(o) => o,
        iddqd::bi_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
    };
    assert!(occupied.is_non_unique());
    let mut entry_mut = occupied.into_mut();
    if let Some(mut r) = entry_mut.by_key1() {
        r.payload = 1111;
    }
    if let Some(mut r) = entry_mut.by_key2() {
        r.payload = 2222;
    }
    drop(entry_mut);

    assert_eq!(map.get1(&3).unwrap().payload, 1111);
    assert_eq!(map.get1(&5).unwrap().payload, 2222);
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

#[test]
fn lying_ord_iter_mut_no_duplicate_yield() {
    let mut map = IdOrdMap::<LyingOrdItem>::new();
    for i in 0..16 {
        map.insert_unique(LyingOrdItem { id: i }).unwrap();
    }
    LIE_ORD.with(|c| c.set(Some(Ordering::Less)));
    let _ = catch_panic(|| {
        for i in 100..110 {
            let _ = map.insert_unique(LyingOrdItem { id: i });
        }
    });

    let mut seen = std::collections::HashSet::new();
    for item in map.iter_mut() {
        assert!(seen.insert(item.id), "iter_mut yielded id={} twice", item.id);
    }
    LIE_ORD.with(|c| c.set(None));
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
    let mut map = IdHashMap::<PayloadItem>::new();
    for i in 0..16 {
        map.insert_unique(PayloadItem { id: i, payload: 0 }).unwrap();
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
        map.insert_unique(LyingOrdItem { id: i }).unwrap();
    }
    LIE_ORD.with(|c| c.set(Some(Ordering::Greater)));
    while catch_panic(|| map.pop_first()).flatten().is_some() {}
    while catch_panic(|| map.pop_last()).flatten().is_some() {}
    LIE_ORD.with(|c| c.set(None));
}
