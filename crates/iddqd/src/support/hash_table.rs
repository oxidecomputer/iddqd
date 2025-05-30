//! A wrapper around a hash table with some random state.

use super::{
    alloc::{AllocWrapper, Allocator},
    map_hash::MapHash,
};
use crate::internal::{TableValidationError, ValidateCompact};
use alloc::{collections::BTreeSet, vec::Vec};
use core::{
    borrow::Borrow,
    fmt,
    hash::{BuildHasher, Hash},
};
use equivalent::Equivalent;
use hashbrown::{
    HashTable,
    hash_table::{AbsentEntry, Entry, OccupiedEntry},
};

#[derive(Clone, Default)]
pub(crate) struct MapHashTable<S, A: Allocator> {
    pub(super) state: S,
    pub(super) items: HashTable<usize, AllocWrapper<A>>,
}

impl<S: fmt::Debug, A: Allocator> fmt::Debug for MapHashTable<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapHashTable")
            .field("state", &self.state)
            .field("items", &self.items)
            .finish()
    }
}

impl<S: Clone + BuildHasher, A: Allocator> MapHashTable<S, A> {
    pub(crate) fn with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        alloc: A,
    ) -> Self {
        Self {
            state: hasher,
            items: HashTable::with_capacity_in(capacity, AllocWrapper(alloc)),
        }
    }

    #[cfg(feature = "daft")]
    pub(crate) fn state(&self) -> &S {
        &self.state
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
                let mut values: Vec<_> = self.items.iter().copied().collect();
                values.sort_unstable();
                for (i, value) in values.iter().enumerate() {
                    if *value != i {
                        return Err(TableValidationError::new(format!(
                            "expected value at index {i} to be {i}, was {value}"
                        )));
                    }
                }
            }
            ValidateCompact::NonCompact => {
                // There should be no duplicates.
                let values: Vec<_> = self.items.iter().copied().collect();
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

    pub(crate) fn compute_hash<K: Hash + Eq>(&self, key: K) -> MapHash<S> {
        MapHash { state: self.state.clone(), hash: self.state.hash_one(key) }
    }

    // Ensure that K has a consistent hash.
    pub(crate) fn find_index<K, Q, F>(
        &self,
        key: &Q,
        lookup: F,
    ) -> Option<usize>
    where
        F: Fn(usize) -> K,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let hash = self.state.hash_one(key);
        self.items.find(hash, |index| key.equivalent(&lookup(*index))).copied()
    }

    pub(crate) fn entry<K: Hash + Eq, F>(
        &mut self,
        key: K,
        lookup: F,
    ) -> Entry<'_, usize, AllocWrapper<A>>
    where
        F: Fn(usize) -> K,
    {
        let hash = self.state.hash_one(&key);
        self.items.entry(
            hash,
            |index| lookup(*index) == key,
            |v| self.state.hash_one(lookup(*v)),
        )
    }

    pub(crate) fn find_entry<K, Q, F>(
        &mut self,
        key: &Q,
        lookup: F,
    ) -> Result<
        OccupiedEntry<'_, usize, AllocWrapper<A>>,
        AbsentEntry<'_, usize, AllocWrapper<A>>,
    >
    where
        F: Fn(usize) -> K,
        K: Hash + Eq + Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let hash = self.state.hash_one(key);
        self.items.find_entry(hash, |index| lookup(*index).borrow() == key)
    }
}
