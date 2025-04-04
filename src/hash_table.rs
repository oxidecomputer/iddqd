// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A wrapper around a hash table with some random state.

use hashbrown::{hash_table::Entry, HashTable};
use std::{
    borrow::Borrow,
    hash::{BuildHasher, Hash, RandomState},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct MapHashTable {
    pub(super) state: RandomState,
    pub(super) entries: HashTable<usize>,
}

impl MapHashTable {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            state: RandomState::new(),
            entries: HashTable::with_capacity(capacity),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn validate(&self, expected_len: usize) -> anyhow::Result<()> {
        use anyhow::ensure;

        ensure!(
            self.len() == expected_len,
            "expected length {expected_len}, was {}",
            self.len()
        );

        // All entries between 0 (inclusive) and self.len() (exclusive) are
        // present, and there are no duplicates.

        let mut values: Vec<_> = self.entries.iter().copied().collect();
        values.sort_unstable();
        for (i, value) in values.iter().enumerate() {
            ensure!(
                *value == i,
                "value at index {i} should be {i}, was {value}",
            );
        }

        Ok(())
    }

    pub(crate) fn compute_hash<K: Hash + Eq>(&self, key: K) -> MapHash {
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
        K: Hash + Eq + Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let hash = self.state.hash_one(key);
        self.entries.find(hash, |index| lookup(*index).borrow() == key).copied()
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
        self.entries.entry(
            hash,
            |index| lookup(*index) == key,
            |v| self.state.hash_one(lookup(*v)),
        )
    }
}

/// Packages up a state and a hash for later validation.
#[derive(Debug)]
pub(crate) struct MapHash {
    state: RandomState,
    hash: u64,
}

impl MapHash {
    pub(crate) fn is_same_hash<K: Hash + Eq>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
