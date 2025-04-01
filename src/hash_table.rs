// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A wrapper around a hash table with some random state.

use std::hash::BuildHasher;
use std::hash::Hash;
use std::hash::RandomState;

use hashbrown::hash_table::Entry;
use hashbrown::HashTable;

#[derive(Clone, Debug, Default)]
pub(crate) struct MapHashTable {
    state: RandomState,
    entries: HashTable<usize>,
}

impl MapHashTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

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
        for i in 0..self.len() {
            ensure!(
                values[i] == i,
                "value at index {i} should be {i}, was {}",
                values[i]
            );
        }

        Ok(())
    }

    // Ensure that K has a consistent hash.
    pub(crate) fn find_index<'a, T, K: Hash + Eq, F>(
        &self,
        entries: &'a [T],
        key: K,
        mut eq: F,
    ) -> Option<usize>
    where
        F: FnMut(&'a T) -> bool,
    {
        let hash = self.state.hash_one(&key);
        self.entries
            .find(hash, |index| eq(&entries[*index]))
            .copied()
    }

    pub(crate) fn entry<K: Hash + Eq, F>(&mut self, key: K, lookup: F) -> Entry<'_, usize>
    where
        F: Fn(usize) -> K,
    {
        let hash = self.state.hash_one(&key);
        self.entries.entry(
            hash,
            |index| lookup(*index) == key,
            |v| {
                let hash = self.state.hash_one(lookup(*v));
                hash
            },
        )
    }
}
