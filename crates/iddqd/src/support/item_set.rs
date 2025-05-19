// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use derive_where::derive_where;
use rustc_hash::FxHashMap;
use std::{
    collections::hash_map,
    ops::{Index, IndexMut},
};

/// A map of items stored by integer index.
#[derive(Clone, Debug)]
#[derive_where(Default)]
pub(crate) struct ItemSet<T> {
    // rustc-hash's FxHashMap is custom-designed for compact-ish integer keys.
    items: FxHashMap<usize, T>,
    // The next index to use. This only ever goes up, not down.
    //
    // An alternative might be to use a free list of indexes, but that's
    // unnecessarily complex.
    next_index: usize,
}

impl<T> ItemSet<T> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            items: FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            ),
            next_index: 0,
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub(crate) fn iter(&self) -> hash_map::Iter<usize, T> {
        self.items.iter()
    }

    #[inline]
    #[expect(dead_code)]
    pub(crate) fn iter_mut(&mut self) -> hash_map::IterMut<usize, T> {
        self.items.iter_mut()
    }

    #[inline]
    pub(crate) fn values(&self) -> hash_map::Values<'_, usize, T> {
        self.items.values()
    }

    #[inline]
    pub(crate) fn values_mut(&mut self) -> hash_map::ValuesMut<'_, usize, T> {
        self.items.values_mut()
    }

    #[inline]
    pub(crate) fn into_values(self) -> hash_map::IntoValues<usize, T> {
        self.items.into_values()
    }

    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.items.get(&index)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(&index)
    }

    #[inline]
    pub(crate) fn next_index(&self) -> usize {
        self.next_index
    }

    #[inline]
    pub(crate) fn insert_at_next_index(&mut self, value: T) -> usize {
        let index = self.next_index;
        self.items.insert(index, value);
        self.next_index += 1;
        index
    }

    #[inline]
    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        let entry = self.items.remove(&index);
        if entry.is_some() && index == self.next_index - 1 {
            // If we removed the last entry, decrement next_index. Not strictly
            // necessary but a nice optimization.
            self.next_index -= 1;
        }
        entry
    }

    // This method assumes that value has the same ID. It also asserts that
    // `index` is valid (and panics if it isn't).
    #[inline]
    pub(crate) fn replace(&mut self, index: usize, value: T) -> T {
        self.items
            .insert(index, value)
            .unwrap_or_else(|| panic!("EntrySet index not found: {index}"))
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::ItemSet;
    use serde::Serialize;

    impl<T: Serialize> Serialize for ItemSet<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            // Serialize just the items -- don't serialize the map keys. We'll
            // rebuild the map keys on deserialization.
            serializer.collect_seq(self.items.values())
        }
    }
}

impl<T> Index<usize> for ItemSet<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.items
            .get(&index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}

impl<T> IndexMut<usize> for ItemSet<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.items
            .get_mut(&index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}
