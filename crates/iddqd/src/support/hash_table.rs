//! A wrapper around a hash table with some random state.

use super::map_hash::MapHash;
use crate::internal::{TableValidationError, ValidateCompact};
use alloc::vec::Vec;
use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
};
use equivalent::Equivalent;
use hashbrown::{
    DefaultHashBuilder, HashTable,
    hash_table::{AbsentEntry, Entry, OccupiedEntry},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct MapHashTable {
    pub(super) state: DefaultHashBuilder,
    pub(super) items: HashTable<usize>,
}

#[cfg(feature = "std")]
fn new_hash_builder() -> DefaultHashBuilder {
    DefaultHashBuilder::default()
}

#[cfg(not(feature = "std"))]
fn new_hash_builder() -> DefaultHashBuilder {
    // Use a default hash builder that doesn't require std.
    DefaultHashBuilder::default()
}

impl MapHashTable {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            state: new_hash_builder(),
            items: HashTable::with_capacity(capacity),
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
        use hashbrown::HashSet;

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
                let value_set: HashSet<_> = values.iter().copied().collect();
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

    pub(crate) fn compute_hash<K: Hash + Eq>(&self, key: K) -> MapHash {
        MapHash { state: self.state, hash: self.state.hash_one(key) }
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
    ) -> Entry<'_, usize>
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
    ) -> Result<OccupiedEntry<'_, usize>, AbsentEntry<'_, usize>>
    where
        F: Fn(usize) -> K,
        K: Hash + Eq + Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let hash = self.state.hash_one(key);
        self.items.find_entry(hash, |index| lookup(*index).borrow() == key)
    }
}
