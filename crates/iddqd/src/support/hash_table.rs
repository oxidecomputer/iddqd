//! A wrapper around a hash table that caches each entry's key hash.
//!
//! # Why we cache the hash
//!
//! Hashbrown's `RawTable::reserve_rehash` may choose to rehash in place when a
//! reserve is requested on a tombstone-heavy table. That path invokes the
//! caller-supplied rehash hasher on every surviving entry, and hashbrown
//! documents it as not panic-safe (a panic mid-rehash can leave the table with
//! duplicate stored values).
//!
//! Duplicate indexes are quite bad!
//!
//! * For ordered maps they immediately lead to mutable aliasing during `iter_mut`.
//! * For hash maps, we don't walk the item set in hash order today, so this
//!   isn't a soundness issue. But this is a very thin guarantee -- a future change
//!   to walk the item set in hash order would easily result in mutable aliasing.
//!
//! By storing the per-entry hash alongside its [`ItemIndex`], we can supply a
//! rehash hasher of the form `|stored| stored.hash` that reads the cached
//! value and never invokes user `Hash`. Rehash is then panic-free by
//! construction.
//!
//! This does add a u64 to every entry, but for the kinds of items iddqd is
//! targeting (fat database records) the overhead is ideally minimal.

use super::{
    ItemIndex,
    alloc::{AllocWrapper, Allocator},
    item_set::IndexRemap,
    map_hash::MapHash,
};
use crate::internal::{TableValidationError, ValidateCompact};
use alloc::{collections::BTreeSet, vec::Vec};
use core::{
    fmt,
    hash::{BuildHasher, Hash},
};
use equivalent::Equivalent;
use hashbrown::{HashTable, hash_table};

/// An [`ItemIndex`] stored in [`MapHashTable`] together with the cached hash
/// of its key.
///
/// See the module docs for why we cache the hash.
#[derive(Clone, Debug)]
pub(crate) struct HashedIndex {
    pub(crate) ix: ItemIndex,
    pub(crate) hash: u64,
}

impl HashedIndex {
    #[inline]
    fn new(ix: ItemIndex, hash: u64) -> Self {
        Self { ix, hash }
    }
}

/// Panic-free rehash hasher. Used everywhere we hand a hasher closure to
/// hashbrown for reserve / shrink / rehash paths.
#[inline]
fn cached_hasher(stored: &HashedIndex) -> u64 {
    stored.hash
}

#[derive(Clone, Default)]
pub(crate) struct MapHashTable<A: Allocator> {
    pub(super) items: HashTable<HashedIndex, AllocWrapper<A>>,
}

impl<A: Allocator> fmt::Debug for MapHashTable<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapHashTable").field("items", &self.items).finish()
    }
}

impl<A: Allocator> MapHashTable<A> {
    pub(crate) const fn new_in(alloc: A) -> Self {
        Self { items: HashTable::new_in(AllocWrapper(alloc)) }
    }

    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: HashTable::with_capacity_in(capacity, AllocWrapper(alloc)),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), TableValidationError> {
        if self.len() != expected_len {
            return Err(TableValidationError::new(format!(
                "expected length {expected_len}, was {}",
                self.len()
            )));
        }

        match compactness {
            ValidateCompact::Compact => {
                // All items between 0 (inclusive) and self.len() (exclusive)
                // are expected to be present, and there are no duplicates.
                let mut values: Vec<_> =
                    self.items.iter().map(|h| h.ix).collect();
                values.sort_unstable();
                for (i, value) in values.iter().enumerate() {
                    if value.as_u32() as usize != i {
                        return Err(TableValidationError::new(format!(
                            "expected value at index {i} to be {i}, was {value}"
                        )));
                    }
                }
            }
            ValidateCompact::NonCompact => {
                // There should be no duplicates.
                let values: Vec<_> = self.items.iter().map(|h| h.ix).collect();
                let value_set: BTreeSet<_> = values.iter().copied().collect();
                if value_set.len() != values.len() {
                    return Err(TableValidationError::new(format!(
                        "expected no duplicates, but found {} duplicates \
                         (values: {:?})",
                        values.len() - value_set.len(),
                        values,
                    )));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn compute_hash<S: BuildHasher, K: Hash + Eq>(
        &self,
        state: &S,
        key: K,
    ) -> MapHash {
        MapHash { hash: state.hash_one(key) }
    }

    // Ensure that K has a consistent hash.
    pub(crate) fn find_index<S: BuildHasher, K, Q, F>(
        &self,
        state: &S,
        key: &Q,
        lookup: F,
    ) -> Option<ItemIndex>
    where
        F: Fn(ItemIndex) -> K,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let hash = state.hash_one(key);
        self.items
            .find(hash, |stored| key.equivalent(&lookup(stored.ix)))
            .map(|stored| stored.ix)
    }

    pub(crate) fn entry<S: BuildHasher, K: Hash + Eq, F>(
        &mut self,
        state: &S,
        key: K,
        lookup: F,
    ) -> Entry<'_, A>
    where
        F: Fn(ItemIndex) -> K,
    {
        let hash = state.hash_one(&key);
        match self.items.entry(
            hash,
            |stored| lookup(stored.ix) == key,
            cached_hasher,
        ) {
            hash_table::Entry::Occupied(inner) => {
                Entry::Occupied(OccupiedEntry { inner })
            }
            hash_table::Entry::Vacant(inner) => {
                Entry::Vacant(VacantEntry { inner, hash })
            }
        }
    }

    pub(crate) fn find_entry_by_hash<F>(
        &mut self,
        hash: u64,
        mut f: F,
    ) -> Result<OccupiedEntry<'_, A>, ()>
    where
        F: FnMut(ItemIndex) -> bool,
    {
        match self.items.find_entry(hash, |stored| f(stored.ix)) {
            Ok(inner) => Ok(OccupiedEntry { inner }),
            Err(_) => Err(()),
        }
    }

    pub(crate) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(ItemIndex) -> bool,
    {
        self.items.retain(|stored| f(stored.ix));
    }

    /// Removes the entry whose stored index is `ix` using a linear scan.
    ///
    /// Used as the cleanup path after a `find_entry_by_hash` miss caused by a
    /// silent key mutation (e.g. `mem::forget` on `RefMut`). The caller has
    /// already identified the `ItemIndex` to remove and needs a removal that
    /// does not re-enter user code.
    ///
    /// The table holds at most one entry per `ItemIndex` (that is the
    /// overarching invariant we're trying to uphold across this crate), so this
    /// removes at most one entry.
    ///
    /// Panics if no such entry exists. Reaching this state means the table and
    /// item set had already diverged before the call, at which point we can no
    /// longer reason about the map.
    pub(crate) fn remove_by_index(&mut self, ix: ItemIndex) {
        let mut found = false;
        self.items.retain(|stored| {
            if !found && stored.ix == ix {
                found = true;
                false
            } else {
                true
            }
        });
        assert!(
            found,
            "linear scan should locate the index that find_entry_by_hash missed"
        );
    }

    /// Clears the hash table, removing all items.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    /// Reserves capacity for at least `additional` more items.
    ///
    /// The rehash closure reads the cached hash on each stored entry (see
    /// the module docs), so this call never invokes user `Hash` even when
    /// hashbrown falls into `rehash_in_place`. That makes `reserve`
    /// panic-safe by construction.
    #[inline]
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional, cached_hasher);
    }

    /// Rewrites every stored index via `remap`.
    ///
    /// Called after [`ItemSet::compact`] compacts the backing items buffer.
    /// We store hashes of *keys* (not of indexes), so rewriting an index does
    /// not invalidate its hash and no rehash is needed.
    ///
    /// [`ItemSet::compact`]: super::item_set::ItemSet::compact
    pub(crate) fn remap_indexes(&mut self, remap: &IndexRemap) {
        for stored in self.items.iter_mut() {
            stored.ix = remap.remap(stored.ix);
        }
    }

    /// Shrinks the capacity of the hash table as much as possible.
    ///
    /// See [`Self::reserve`] for why the rehash closure is panic-free.
    #[inline]
    pub(crate) fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit(cached_hasher);
    }

    /// Shrinks the capacity of the hash table with a lower limit.
    ///
    /// See [`Self::reserve`] for why the rehash closure is panic-free.
    #[inline]
    pub(crate) fn shrink_to(&mut self, min_capacity: usize) {
        self.items.shrink_to(min_capacity, cached_hasher);
    }

    /// Tries to reserve capacity for at least `additional` more items.
    ///
    /// See [`Self::reserve`] for why the rehash closure is panic-free.
    #[inline]
    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), hashbrown::TryReserveError> {
        self.items.try_reserve(additional, cached_hasher)
    }

    /// Test-only variant of [`Self::reserve`] that returns how many times the
    /// rehash closure was invoked.
    ///
    /// A non-zero return value proves the rehash callback fired, so the
    /// caller's setup actually exercises a rehash path rather than landing
    /// in hashbrown's "no-op, growth_left already sufficient" branch. The
    /// closure delegates to [`cached_hasher`], so this exercises the real
    /// production hasher.
    #[cfg(all(test, feature = "std"))]
    pub(crate) fn reserve_counting_rehash(
        &mut self,
        additional: usize,
    ) -> usize {
        let count = core::cell::Cell::new(0usize);
        self.items.reserve(additional, |stored| {
            count.set(count.get() + 1);
            cached_hasher(stored)
        });
        count.into_inner()
    }
}

/// An entry in [`MapHashTable`].
///
/// Wraps hashbrown's `hash_table::Entry` to keep the cached-hash bookkeeping
/// inside this module: callers see [`ItemIndex`] only.
pub(crate) enum Entry<'a, A: Allocator> {
    Occupied(OccupiedEntry<'a, A>),
    Vacant(VacantEntry<'a, A>),
}

pub(crate) struct OccupiedEntry<'a, A: Allocator> {
    inner: hash_table::OccupiedEntry<'a, HashedIndex, AllocWrapper<A>>,
}

impl<'a, A: Allocator> OccupiedEntry<'a, A> {
    /// Returns the [`ItemIndex`] stored in this entry.
    #[inline]
    pub(crate) fn get(&self) -> ItemIndex {
        self.inner.get().ix
    }

    /// Removes this entry from the table.
    #[inline]
    pub(crate) fn remove(self) {
        let _ = self.inner.remove();
    }
}

pub(crate) struct VacantEntry<'a, A: Allocator> {
    inner: hash_table::VacantEntry<'a, HashedIndex, AllocWrapper<A>>,
    /// The hash used to obtain this `VacantEntry`.
    hash: u64,
}

impl<'a, A: Allocator> VacantEntry<'a, A> {
    /// Inserts the given index with the hash captured when this entry was
    /// obtained.
    #[inline]
    pub(crate) fn insert(self, ix: ItemIndex) {
        let _ = self.inner.insert(HashedIndex::new(ix, self.hash));
    }
}
