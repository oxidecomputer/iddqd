// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use derive_where::derive_where;
use rustc_hash::FxHashMap;
use std::{
    collections::hash_map,
    ops::{Index, IndexMut},
};

/// A map of entries stored by integer index.
#[derive(Clone, Debug)]
#[derive_where(Default)]
pub(crate) struct EntrySet<T> {
    // rustc-hash's FxHashMap is custom-designed for compact-ish integer keys.
    entries: FxHashMap<usize, T>,
    // The next index to use. This only ever goes up, not down.
    //
    // An alternative might be to use a free list of indexes, but that's
    // unnecessarily complex.
    next_index: usize,
}

impl<T> EntrySet<T> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            ),
            next_index: 0,
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn iter(&self) -> hash_map::Iter<usize, T> {
        self.entries.iter()
    }

    #[inline]
    #[expect(dead_code)]
    pub(crate) fn iter_mut(&mut self) -> hash_map::IterMut<usize, T> {
        self.entries.iter_mut()
    }

    #[inline]
    pub(crate) fn values(&self) -> hash_map::Values<'_, usize, T> {
        self.entries.values()
    }

    #[inline]
    pub(crate) fn values_mut(&mut self) -> hash_map::ValuesMut<'_, usize, T> {
        self.entries.values_mut()
    }

    #[inline]
    pub(crate) fn into_values(self) -> hash_map::IntoValues<usize, T> {
        self.entries.into_values()
    }

    #[inline]
    #[expect(dead_code)]
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.entries.get(&index)
    }

    #[inline]
    #[expect(dead_code)]
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.entries.get_mut(&index)
    }

    #[inline]
    pub(crate) fn insert(&mut self, value: T) -> usize {
        let index = self.next_index;
        self.entries.insert(index, value);
        self.next_index += 1;
        index
    }

    /// Converts self into a `Vec<T>` sorted by index.
    #[cfg(test)]
    pub(crate) fn into_vec(mut self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.entries.len());
        for i in 0..self.next_index {
            // This is slightly inefficient in the face of lots of gaps in
            // self.entries, but it is also test-only code.
            if let Some(entry) = self.entries.remove(&i) {
                vec.push(entry);
            }
        }
        vec
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::EntrySet;
    use serde::Serialize;

    impl<T: Serialize> Serialize for EntrySet<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            // Serialize just the entries -- don't serialize the map keys. We'll
            // rebuild the map keys on deserialization.
            serializer.collect_seq(self.entries.values())
        }
    }
}

impl<T> Index<usize> for EntrySet<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.entries
            .get(&index)
            .unwrap_or_else(|| panic!("EntrySet index not found: {index}"))
    }
}

impl<T> IndexMut<usize> for EntrySet<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.entries
            .get_mut(&index)
            .unwrap_or_else(|| panic!("EntrySet index not found: {index}"))
    }
}
