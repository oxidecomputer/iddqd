//! A "table" of b-tree-based indexes.
//!
//! Similar to [`super::hash_table::MapHashTable`], b-tree based tables store
//! integers (that are indexes corresponding to items), but use an external
//! comparator.

use super::{ItemIndex, item_set::IndexRemap, map_hash::MapHash};
use crate::internal::{TableValidationError, ValidateCompact};
use alloc::{
    collections::{BTreeSet, btree_set},
    vec::Vec,
};
use core::{
    cell::Cell,
    cmp::Ordering,
    hash::{BuildHasher, Hash},
    marker::PhantomData,
};
use equivalent::Comparable;

thread_local! {
    /// Stores an external comparator function to provide dynamic scoping.
    ///
    /// std's BTreeMap doesn't allow passing an external comparator, so we make
    /// do with this function that's passed in through dynamic scoping.
    ///
    /// This works by:
    ///
    /// * We store an `Index` in the BTreeSet which knows how to call this
    ///   dynamic comparator.
    /// * When we need to compare two `Index` values, we create a CmpDropGuard.
    ///   This struct is responsible for managing the lifetime of the
    ///   comparator.
    /// * When the CmpDropGuard is dropped (including due to a panic), we reset
    ///   the comparator to None.
    ///
    /// Comparators take `&Index` rather than `Index` by value because `Index`
    /// wraps `IndexCell` (an `AtomicU32` newtype) for in-place mutation in
    /// `remap_indexes`, and `AtomicU32` isn't `Copy`.
    ///
    /// This is not great! (For one, thread-locals and no-std don't really mix.)
    /// Some alternatives:
    ///
    /// * Using `Borrow` as described in
    ///   https://github.com/sunshowers-code/borrow-complex-key-example. While
    ///   hacky, this actually works for the find operation. But the insert
    ///   operation currently requires a concrete `Index`.
    ///
    ///   If and when https://github.com/rust-lang/rust/issues/133549 lands,
    ///   this should become a viable option. Worth looking out for!
    ///
    /// * Using a third-party BTreeSet implementation that allows passing in
    ///   external comparators. As of 2025-05, there appear to be two options:
    ///
    ///   1. copse (https://docs.rs/copse), which doesn't seem like a good fit
    ///      here.
    ///   2. btree_monstrousity (https://crates.io/crates/btree_monstrousity),
    ///      which has an API perfect for this but is, uhh, not really
    ///      production-ready.
    ///
    ///   Third-party implementations also run the risk of being relatively
    ///   untested.
    ///
    /// * Using some other kind of sorted set. We've picked B-trees here as the
    ///   default choice to balance cache locality, but other options are worth
    ///   benchmarking. We do need to provide a comparator, though, so radix
    ///   trees and such are out of the question.
    static CMP: Cell<Option<&'static IndexCmp<'static>>>
        = const { Cell::new(None) };
}

/// External comparator type used via `CMP`'s dynamic scoping.
type IndexCmp<'a> = dyn Fn(&Index, &Index) -> Ordering + 'a;

/// A B-tree-based table with an external comparator.
#[derive(Clone, Debug, Default)]
pub(crate) struct MapBTreeTable {
    items: BTreeSet<Index>,
    // We use foldhash directly here because we allow compiling with std but
    // without the default-hasher. std turns on foldhash but not the default
    // hasher.
    hash_state: foldhash::fast::FixedState,
}

impl MapBTreeTable {
    pub(crate) const fn new() -> Self {
        Self {
            items: BTreeSet::new(),
            // FixedState::with_seed XORs the passed in seed with a fixed
            // high-entropy value.
            hash_state: foldhash::fast::FixedState::with_seed(0),
        }
    }

    #[doc(hidden)]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[doc(hidden)]
    pub(crate) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), TableValidationError> {
        if self.len() != expected_len {
            return Err(TableValidationError::new(format!(
                "expected length {expected_len}, was {}",
                self.len(),
            )));
        }

        match compactness {
            ValidateCompact::Compact => {
                // All items between 0 (inclusive) and self.len() (exclusive)
                // are present, and there are no duplicates. Also, the sentinel
                // value should not be stored.
                let mut indexes: Vec<ItemIndex> =
                    Vec::with_capacity(expected_len);
                for index in &self.items {
                    let v = index.value();
                    if v == Index::SENTINEL_VALUE {
                        return Err(TableValidationError::new(
                            "sentinel value should not be stored in map",
                        ));
                    }
                    indexes.push(v);
                }
                indexes.sort_unstable();
                for (i, index) in indexes.iter().enumerate() {
                    if index.as_u32() as usize != i {
                        return Err(TableValidationError::new(format!(
                            "value at index {i} should be {i}, was {index}",
                        )));
                    }
                }
            }
            ValidateCompact::NonCompact => {
                // There should be no duplicates, and the sentinel value
                // should not be stored.
                let indexes: Vec<ItemIndex> =
                    self.items.iter().map(|ix| ix.value()).collect();
                let index_set: BTreeSet<ItemIndex> =
                    indexes.iter().copied().collect();
                if index_set.len() != indexes.len() {
                    return Err(TableValidationError::new(format!(
                        "expected no duplicates, but found {} duplicates \
                         (values: {:?})",
                        indexes.len() - index_set.len(),
                        indexes,
                    )));
                }
                if index_set.contains(&Index::SENTINEL_VALUE) {
                    return Err(TableValidationError::new(
                        "sentinel value should not be stored in map",
                    ));
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn first(&self) -> Option<ItemIndex> {
        self.items.first().map(|ix| ix.value())
    }

    #[inline]
    pub(crate) fn last(&self) -> Option<ItemIndex> {
        self.items.last().map(|ix| ix.value())
    }

    pub(crate) fn find_index<K, Q, F>(
        &self,
        key: &Q,
        lookup: F,
    ) -> Option<ItemIndex>
    where
        K: Ord,
        Q: ?Sized + Comparable<K>,
        F: Fn(ItemIndex) -> K,
    {
        let f = find_cmp(key, lookup);

        let guard = CmpDropGuard::new(&f);

        let ret = match self.items.get(&Index::sentinel()) {
            Some(ix) if ix.value() == Index::SENTINEL_VALUE => {
                panic!("internal map shouldn't store sentinel value")
            }
            Some(ix) => Some(ix.value()),
            None => {
                // The key is not in the table.
                None
            }
        };

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
        ret
    }

    pub(crate) fn insert<K, Q, F>(
        &mut self,
        index: ItemIndex,
        key: &Q,
        lookup: F,
    ) where
        K: Ord,
        Q: ?Sized + Comparable<K>,
        F: Fn(ItemIndex) -> K,
    {
        let f = insert_cmp(index, key, lookup);
        let guard = CmpDropGuard::new(&f);

        self.items.insert(Index::new(index));

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
    }

    pub(crate) fn remove<K, F>(&mut self, index: ItemIndex, key: K, lookup: F)
    where
        F: Fn(ItemIndex) -> K,
        K: Ord,
    {
        let f = insert_cmp(index, &key, lookup);
        let guard = CmpDropGuard::new(&f);

        self.items.remove(&Index::new(index));

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
    }

    pub(crate) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(ItemIndex) -> bool,
    {
        // We don't need to set up a comparator in the environment because
        // `retain` doesn't do any comparisons as part of its operation.
        self.items.retain(|index| f(index.value()));
    }

    /// Rewrites every stored index via `remap`.
    ///
    /// Called after [`ItemSet::shrink_to_fit`] or [`ItemSet::shrink_to`]
    /// compacts the backing items buffer. Each stored `Index` needs to be
    /// rewritten to point at the item's new position.
    ///
    /// We do not rebuild the tree. [`IndexRemap`] preserves relative
    /// order, so the tree's iteration order — which is the user's
    /// `Ord` over items — matches before and after the rewrite. Only
    /// the stored index values change; node structure, pointers, and
    /// the user-visible total order are all preserved. The walk is
    /// O(N) with no comparator calls and no allocations.
    ///
    /// In-place mutation through `&Index` is provided by [`IndexCell`],
    /// which uses an `AtomicU32` for `&self`-based stores.
    ///
    /// [`ItemSet::shrink_to_fit`]: super::item_set::ItemSet::shrink_to_fit
    /// [`ItemSet::shrink_to`]: super::item_set::ItemSet::shrink_to
    pub(crate) fn remap_indexes(&mut self, remap: &IndexRemap) {
        for idx in self.items.iter() {
            let new = remap.remap(idx.value());
            idx.set_value(new);
        }
    }

    /// Clears the B-tree table, removing all items.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    pub(crate) fn iter(&self) -> Iter<'_> {
        Iter::new(self.items.iter())
    }

    pub(crate) fn into_iter(self) -> IntoIter {
        IntoIter::new(self.items.into_iter())
    }

    pub(crate) fn state(&self) -> &foldhash::fast::FixedState {
        &self.hash_state
    }

    pub(crate) fn compute_hash<K: Hash>(&self, key: K) -> MapHash {
        MapHash { hash: self.hash_state.hash_one(key) }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Iter<'a> {
    inner: btree_set::Iter<'a, Index>,
}

impl<'a> Iter<'a> {
    fn new(inner: btree_set::Iter<'a, Index>) -> Self {
        Self { inner }
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = ItemIndex;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|index| index.value())
    }
}

#[derive(Debug)]
pub(crate) struct IntoIter {
    inner: btree_set::IntoIter<Index>,
}

impl IntoIter {
    fn new(inner: btree_set::IntoIter<Index>) -> Self {
        Self { inner }
    }
}

impl Iterator for IntoIter {
    type Item = ItemIndex;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|index| index.value())
    }
}

fn find_cmp<'a, K, Q, F>(
    key: &'a Q,
    lookup: F,
) -> impl Fn(&Index, &Index) -> Ordering + 'a
where
    Q: ?Sized + Comparable<K>,
    F: 'a + Fn(ItemIndex) -> K,
    K: Ord,
{
    move |a: &Index, b: &Index| {
        let (a, b) = (a.value(), b.value());
        if a == b {
            // This is potentially load-bearing! It means that even if the Eq
            // implementation on map items is wrong, we treat items at the same
            // index as equal.
            //
            // Unsafe code relies on this to ensure that we don't return
            // multiple mutable references to the same index.
            return Ordering::Equal;
        }
        match (a, b) {
            (Index::SENTINEL_VALUE, v) => key.compare(&lookup(v)),
            (v, Index::SENTINEL_VALUE) => key.compare(&lookup(v)).reverse(),
            (a, b) => lookup(a).cmp(&lookup(b)),
        }
    }
}

fn insert_cmp<'a, K, Q, F>(
    index: ItemIndex,
    key: &'a Q,
    lookup: F,
) -> impl Fn(&Index, &Index) -> Ordering + 'a
where
    Q: ?Sized + Comparable<K>,
    F: 'a + Fn(ItemIndex) -> K,
    K: Ord,
{
    move |a: &Index, b: &Index| {
        let (a, b) = (a.value(), b.value());
        if a == b {
            // This is potentially load-bearing! It means that even if the Eq
            // implementation on map items is wrong, we treat items at the same
            // index as equal.
            //
            // Unsafe code relies on this to ensure that we don't return
            // multiple mutable references to the same index.
            return Ordering::Equal;
        }
        match (a, b) {
            // The sentinel value should not be invoked at all, because it's not
            // passed in during insert and not stored in the table.
            (Index::SENTINEL_VALUE, _) | (_, Index::SENTINEL_VALUE) => {
                panic!("sentinel value should not be invoked in insert path")
            }
            (a, b) if a == index => key.compare(&lookup(b)),
            (a, b) if b == index => key.compare(&lookup(a)).reverse(),
            (a, b) => lookup(a).cmp(&lookup(b)),
        }
    }
}

struct CmpDropGuard<'a> {
    _marker: PhantomData<&'a ()>,
}

impl<'a> CmpDropGuard<'a> {
    fn new(f: &'a IndexCmp<'a>) -> Self {
        // CMP lasts only as long as this function and is immediately reset to
        // None once this scope is left.
        let ret = Self { _marker: PhantomData };

        // SAFETY: This is safe because we are not storing the reference
        // anywhere, and it is only used for the lifetime of this CmpDropGuard.
        let as_static = unsafe {
            std::mem::transmute::<&'a IndexCmp<'a>, &'static IndexCmp<'static>>(
                f,
            )
        };
        CMP.set(Some(as_static));

        ret
    }
}

impl Drop for CmpDropGuard<'_> {
    fn drop(&mut self) {
        CMP.set(None);
    }
}

/// An [`ItemIndex`] (= `u32`) with interior mutability, layout-identical
/// to `u32`.
///
/// Backed by `AtomicU32`. We use `Relaxed` ordering everywhere because
/// the only caller of `set` holds `&mut MapBTreeTable`, which excludes
/// every other reference — so there is never a race between a reader
/// and a writer. `Relaxed` loads compile to a plain `mov` on x86-64
/// and similar instructions on other architectures, so this gives us
/// interior mutability at the cost of a normal load.
///
/// Going through `AtomicU32` rather than `Cell<u32>` keeps us
/// naturally `Sync` without an `unsafe impl Sync` — `AtomicU32` is
/// designed to be accessed from multiple threads.
#[repr(transparent)]
#[derive(Debug, Default)]
struct IndexCell(core::sync::atomic::AtomicU32);

impl Clone for IndexCell {
    fn clone(&self) -> Self {
        Self(core::sync::atomic::AtomicU32::new(self.get().as_u32()))
    }
}

impl IndexCell {
    #[inline]
    const fn new(value: ItemIndex) -> Self {
        Self(core::sync::atomic::AtomicU32::new(value.as_u32()))
    }

    #[inline]
    fn get(&self) -> ItemIndex {
        ItemIndex::new(self.0.load(core::sync::atomic::Ordering::Relaxed))
    }

    /// Overwrite the stored value. The atomic store makes this safe to
    /// call through `&self`, though in practice callers only invoke it
    /// while holding `&mut` on the enclosing `MapBTreeTable`.
    #[inline]
    fn set(&self, value: ItemIndex) {
        debug_assert_ne!(
            value,
            ItemIndex::SENTINEL,
            "IndexCell::set: sentinel must never be stored in the table",
        );
        self.0.store(value.as_u32(), core::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone, Debug)]
struct Index(IndexCell);

impl Index {
    const SENTINEL_VALUE: ItemIndex = ItemIndex::SENTINEL;

    /// Returns a fresh sentinel `Index`.
    ///
    /// A function rather than an associated `const` because `IndexCell`
    /// wraps an `AtomicU32` (interior mutability), and a `const Self`
    /// would trigger `clippy::declare_interior_mutable_const` at every
    /// borrow site.
    #[inline]
    const fn sentinel() -> Self {
        Self(IndexCell::new(Self::SENTINEL_VALUE))
    }

    #[inline]
    fn new(value: ItemIndex) -> Self {
        if value == Self::SENTINEL_VALUE {
            panic!("btree map overflow, index with value {value:?} was added")
        }
        Self(IndexCell::new(value))
    }

    #[inline]
    fn value(&self) -> ItemIndex {
        self.0.get()
    }

    /// Overwrite the stored index value in place.
    ///
    /// Safe thanks to the atomic store inside [`IndexCell::set`]. In
    /// practice we only call this from `remap_indexes`, which holds
    /// `&mut MapBTreeTable`.
    #[inline]
    fn set_value(&self, value: ItemIndex) {
        self.0.set(value)
    }
}

impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        // For non-sentinel indexes, two values are the same iff their indexes
        // are the same. This is ensured by the fact that our key types
        // implement Eq (as part of implementing Ord).
        let (a, b) = (self.value(), other.value());
        if a != Self::SENTINEL_VALUE && b != Self::SENTINEL_VALUE {
            return a == b;
        }

        // If any of the two indexes is the sentinel, we're required to perform
        // a lookup.
        CMP.with(|cmp| {
            let cmp = cmp.get().expect("cmp should be set");
            cmp(self, other) == Ordering::Equal
        })
    }
}

impl Eq for Index {}

impl Ord for Index {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // Ord should only be called if we're doing lookups within the table,
        // which should have set the thread local.
        CMP.with(|cmp| {
            let cmp = cmp.get().expect("cmp should be set");
            cmp(self, other)
        })
    }
}

impl PartialOrd for Index {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::support::{alloc::Global, item_set::ItemSet};
    use core::cell::Cell;

    thread_local! {
        /// When set, `PanickingKey::cmp` panics on invocation. Scoped by
        /// individual tests.
        static PANIC_TRIGGER: Cell<bool> = const { Cell::new(false) };
    }

    /// A key type whose `Ord` impl can be made to panic on demand.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct PanickingKey(u32);

    impl PartialOrd for PanickingKey {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for PanickingKey {
        fn cmp(&self, other: &Self) -> Ordering {
            if PANIC_TRIGGER.with(|c| c.get()) {
                panic!("simulated Ord panic");
            }
            self.0.cmp(&other.0)
        }
    }

    /// `remap_indexes` must not invoke the user-supplied `Ord` impl. We arm
    /// `PANIC_TRIGGER` for the duration of the call and verify the rebuild
    /// succeeds — any stray user-Ord invocation would panic the test.
    #[test]
    fn remap_indexes_does_not_call_user_ord() {
        // Build an IndexRemap with holes. This yields holes = [1, 3].
        let mut set: ItemSet<PanickingKey, Global> = ItemSet::new();
        for i in 0..5u32 {
            set.assert_can_grow().insert(PanickingKey(i * 10));
        }
        set.remove(ItemIndex::new(1));
        set.remove(ItemIndex::new(3));
        let remap = set.shrink_to_fit();
        assert!(!remap.is_identity(), "remap should carry two holes");

        // A MapBTreeTable populated to match the pre-compaction live indexes
        // 0, 2, 4 — these are the indexes the outer map would have stored
        // before shrink. Setup uses the user `Ord`, so the trigger is off.
        let mut table = MapBTreeTable::new();
        let pre_lookup = |ix: ItemIndex| -> PanickingKey {
            match ix.as_u32() {
                0 => PanickingKey(0),
                2 => PanickingKey(20),
                4 => PanickingKey(40),
                _ => panic!("unexpected index in pre-compaction lookup: {ix}"),
            }
        };
        for ix in [0u32, 2, 4] {
            let ix = ItemIndex::new(ix);
            let key = pre_lookup(ix);
            table.insert(ix, &key, pre_lookup);
        }
        assert_eq!(table.len(), 3);
        assert_eq!(
            table
                .items
                .iter()
                .map(|i| i.value().as_u32())
                .collect::<alloc::vec::Vec<_>>(),
            [0u32, 2, 4],
        );

        // Arm the trigger: any call into `PanickingKey::cmp` during the
        // rebuild below will panic this test.
        PANIC_TRIGGER.with(|c| c.set(true));
        table.remap_indexes(&remap);
        PANIC_TRIGGER.with(|c| c.set(false));

        // Remap 0 -> 0, 2 -> 1, 4 -> 2, and key order is preserved, so the
        // final contents must be [0, 1, 2].
        assert_eq!(
            table
                .items
                .iter()
                .map(|i| i.value().as_u32())
                .collect::<alloc::vec::Vec<_>>(),
            [0u32, 1, 2],
        );
    }
}
