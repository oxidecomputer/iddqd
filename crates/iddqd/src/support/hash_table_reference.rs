//! A `Vec`-backed reference hash table, used under `cfg(soteria)`.
//!
//! # Why this exists
//!
//! Soteria's symbolic memory model cannot execute hashbrown's SwissTable: the
//! control-byte group loads read memory that the engine treats as
//! uninitialized/out of bounds, so any proof that reaches hashbrown cannot
//! proceed past it. (Other model checkers like Kani's CBMC backend have the
//! same issue.)
//!
//! Under `cfg(soteria)`, [`MapHashTable`] swaps hashbrown's `HashTable` for the
//! linear-scan [`HashTable`] in this module. Effectively, we assume that
//! hashbrown is correct and restrict Soteria's verification to iddqd itself, on
//! top of a table that is obviously correct.
//!
//! # Fidelity
//!
//! This type mirrors the slice of hashbrown's `HashTable` API that
//! [`MapHashTable`] calls. Each entry is stored as `(insert_hash, value)`, and
//! looking for items matches on `entry.insert_hash == lookup_hash &&
//! eq(value)`.
//!
//! Real hashbrown uses the control byte (the top bits of the insert hash)
//! before calling `eq`, so a lookup whose hash differs from the insert hash
//! *misses*. Filtering on the full hash models exactly that: an inconsistent
//! user `Hash` produces a realistic lookup miss, which keeps the hash
//! load-bearing. This is more pessimistic than hashbrown, since that can
//! (rarely) return an accidental hit anyway. But we're interested in verifying
//! iddqd's redundant-insert and cleanup paths here, so being more pessimistic
//! than hashbrown is acceptable.
//!
//! OOM is covered separately by tests, not by these proofs.
//!
//! [`IdHashMap`]: crate::IdHashMap
//! [`MapHashTable`]: super::hash_table::MapHashTable

use allocator_api2::{alloc::Allocator, vec::Vec};
use core::fmt;

pub(crate) mod hash_table {
    pub(crate) use super::{Entry, OccupiedEntry, VacantEntry};
}

pub(crate) struct HashTable<T, A: Allocator> {
    /// Each entry is `(insert_hash, value)`. See the module docs for why the
    /// hash is stored alongside the value.
    entries: Vec<(u64, T), A>,
}

impl<T, A: Allocator> HashTable<T, A> {
    #[inline]
    pub(crate) const fn new_in(alloc: A) -> Self {
        Self { entries: Vec::new_in(alloc) }
    }

    #[inline]
    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self { entries: Vec::with_capacity_in(capacity, alloc) }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(_, value)| value)
    }

    #[inline]
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entries.iter_mut().map(|(_, value)| value)
    }

    /// Returns the first entry whose insert hash equals `hash` and that
    /// satisfies `eq`.
    pub(crate) fn find(
        &self,
        hash: u64,
        mut eq: impl FnMut(&T) -> bool,
    ) -> Option<&T> {
        self.entries
            .iter()
            .find(|(stored_hash, value)| *stored_hash == hash && eq(value))
            .map(|(_, value)| value)
    }

    /// Returns an occupied or vacant entry for `hash` + `eq`, mirroring
    /// `hashbrown::HashTable::entry`.
    ///
    /// The `hasher` argument (hashbrown's rehash closure) is unused, since a
    /// linear-scan table never rehashes.
    pub(crate) fn entry(
        &mut self,
        hash: u64,
        mut eq: impl FnMut(&T) -> bool,
        _hasher: impl Fn(&T) -> u64,
    ) -> Entry<'_, T, A> {
        match self.position(hash, &mut eq) {
            Some(index) => {
                Entry::Occupied(OccupiedEntry { table: self, index })
            }
            None => Entry::Vacant(VacantEntry { table: self, hash }),
        }
    }

    /// Returns the occupied entry for `hash` + `eq`, or `Err(())` if absent.
    pub(crate) fn find_entry(
        &mut self,
        hash: u64,
        mut eq: impl FnMut(&T) -> bool,
    ) -> Result<OccupiedEntry<'_, T, A>, ()> {
        match self.position(hash, &mut eq) {
            Some(index) => Ok(OccupiedEntry { table: self, index }),
            None => Err(()),
        }
    }

    pub(crate) fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.entries.retain(|(_, value)| f(value));
    }

    /// Inserts `value` with `hash` without checking for an existing match.
    ///
    /// The `hasher` argument (hashbrown's rehash closure) is unused, since a
    /// linear-scan table never rehashes.
    pub(crate) fn insert_unique(
        &mut self,
        hash: u64,
        value: T,
        _hasher: impl Fn(&T) -> u64,
    ) -> OccupiedEntry<'_, T, A> {
        self.entries.push((hash, value));
        let index = self.entries.len() - 1;
        OccupiedEntry { table: self, index }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    #[inline]
    pub(crate) fn reserve(
        &mut self,
        additional: usize,
        _hasher: impl Fn(&T) -> u64,
    ) {
        self.entries.reserve(additional);
    }

    #[inline]
    pub(crate) fn shrink_to_fit(&mut self, _hasher: impl Fn(&T) -> u64) {
        self.entries.shrink_to_fit();
    }

    #[inline]
    pub(crate) fn shrink_to(
        &mut self,
        min_capacity: usize,
        _hasher: impl Fn(&T) -> u64,
    ) {
        self.entries.shrink_to(min_capacity);
    }

    /// Always succeeds (allocation failure is out of scope for this module).
    #[inline]
    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
        _hasher: impl Fn(&T) -> u64,
    ) -> Result<(), hashbrown::TryReserveError> {
        self.entries.reserve(additional);
        Ok(())
    }

    /// Index of the first entry matching `hash` + `eq`.
    fn position(
        &self,
        hash: u64,
        eq: &mut impl FnMut(&T) -> bool,
    ) -> Option<usize> {
        self.entries
            .iter()
            .position(|(stored_hash, value)| *stored_hash == hash && eq(value))
    }
}

impl<T: Clone, A: Allocator + Clone> Clone for HashTable<T, A> {
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone() }
    }
}

impl<T, A: Allocator + Default> Default for HashTable<T, A> {
    fn default() -> Self {
        Self { entries: Vec::new_in(A::default()) }
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for HashTable<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Mirror of `hashbrown::hash_table::Entry`.
pub(crate) enum Entry<'a, T, A: Allocator> {
    Occupied(OccupiedEntry<'a, T, A>),
    Vacant(VacantEntry<'a, T, A>),
}

/// Mirror of `hashbrown::hash_table::OccupiedEntry`.
pub(crate) struct OccupiedEntry<'a, T, A: Allocator> {
    table: &'a mut HashTable<T, A>,
    index: usize,
}

impl<'a, T, A: Allocator> OccupiedEntry<'a, T, A> {
    #[inline]
    pub(crate) fn get(&self) -> &T {
        &self.table.entries[self.index].1
    }

    /// Removes and returns the entry's value. `swap_remove` keeps this O(1);
    /// the table is an unordered set, so reordering is immaterial.
    #[inline]
    pub(crate) fn remove(self) -> T {
        self.table.entries.swap_remove(self.index).1
    }
}

/// Mirror of `hashbrown::hash_table::VacantEntry`.
pub(crate) struct VacantEntry<'a, T, A: Allocator> {
    table: &'a mut HashTable<T, A>,
    /// The lookup hash that produced this vacant entry; stored with the value
    /// on insert so subsequent lookups with the same hash hit.
    hash: u64,
}

impl<'a, T, A: Allocator> VacantEntry<'a, T, A> {
    #[inline]
    pub(crate) fn insert(self, value: T) -> OccupiedEntry<'a, T, A> {
        self.table.entries.push((self.hash, value));
        let index = self.table.entries.len() - 1;
        OccupiedEntry { table: self.table, index }
    }
}
