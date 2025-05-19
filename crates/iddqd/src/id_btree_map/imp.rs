// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{
    tables::IdBTreeMapTables, Entry, IdOrdItem, IdOrdItemMut, IntoIter, Iter,
    IterMut, OccupiedEntry, RefMut, VacantEntry,
};
use crate::{
    errors::DuplicateItem,
    support::{borrow::DormantMutRef, item_set::ItemSet},
};
use derive_where::derive_where;
use std::{borrow::Borrow, collections::BTreeSet};

/// An ordered map where the keys are part of the values, based on a B-Tree.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in three b-tree maps. This allows for efficient lookups
/// by any of the three keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone, Debug)]
pub struct IdBTreeMap<T: IdOrdItem> {
    pub(super) items: ItemSet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: IdBTreeMapTables,
}

impl<T: IdOrdItem> IdBTreeMap<T> {
    /// Creates a new, empty `IdBTreeMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: IdBTreeMapTables::new() }
    }

    /// Constructs a new `IdBTreeMap` from an iterator of values, rejecting
    /// duplicates.
    ///
    /// To overwrite duplicates instead, use [`IdBTreeMap::from_iter`].
    pub fn from_iter_unique<I: IntoIterator<Item = T>>(
        iter: I,
    ) -> Result<Self, DuplicateItem<T>> {
        let mut map = IdBTreeMap::new();
        for value in iter {
            match map.entry(value.key()) {
                Entry::Occupied(entry) => {
                    let duplicate = entry.remove();
                    return Err(DuplicateItem::__internal_new(
                        value,
                        vec![duplicate],
                    ));
                }
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
            }
        }

        Ok(map)
    }

    /// Returns true if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of items in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterates over the items in the map.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.items, &self.tables)
    }

    /// Iterates over the items in the map, allowing for mutation.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T>
    where
        T: IdOrdItemMut,
    {
        IterMut::new(&mut self.items, &self.tables)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[doc(hidden)]
    // TODO: replace anyhow
    pub fn validate(
        &self,
        compactness: crate::internal::ValidateCompact,
    ) -> anyhow::Result<()>
    where
        T: std::fmt::Debug,
    {
        use anyhow::Context;

        self.tables.validate(self.items.len(), compactness)?;

        // Check that the indexes are all correct.
        for (&ix, item) in self.items.iter() {
            let key = item.key();

            let ix1 = self.find_index(&key).with_context(|| {
                format!("item at index {ix} has no key index")
            })?;

            if ix1 != ix {
                return Err(anyhow::anyhow!(
                    "item at index {ix} has mismatched indexes: {} != {}",
                    ix,
                    ix1,
                ));
            }
        }

        Ok(())
    }

    /// Inserts a value into the set, returning an error if any duplicates were
    /// added.
    pub fn insert_unique(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateItem<T, &T>> {
        let _ = self.insert_unique_impl(value)?;
        Ok(())
    }

    /// Inserts a value into the map, removing and returning the conflicting
    /// item, if any.
    pub fn insert_overwrite(&mut self, value: T) -> Option<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove the item corresponding to the key that is already in the map.
        // 2. Add the item to the map.

        let duplicate = self.remove(value.key());

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicate
    }

    /// Returns true if the map contains the given `key`.
    pub fn contains_key<'a, Q>(&'a self, key: &Q) -> bool
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find_index(key).is_some()
    }

    /// Gets a reference to the value associated with the given `key`.
    pub fn get<'a, Q>(&'a self, key: &Q) -> Option<&'a T>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find(key)
    }

    /// Gets a mutable reference to the item associated with the given `key`.
    ///
    /// Due to borrow checker limitations, this always accepts `T::Key` rather
    /// than a borrowed form of it.
    pub fn get_mut<'a>(&'a mut self, key: T::Key<'_>) -> Option<RefMut<'a, T>>
    where
        T: IdOrdItemMut,
    {
        let index = self.find_index(&T::upcast_key(key))?;
        let item = &mut self.items[index];
        Some(RefMut::new(item))
    }

    /// Removes an item from the map by its `key`.
    ///
    /// Due to borrow checker limitations, this always accepts `T::Key` rather
    /// than a borrowed form of it.
    pub fn remove(&mut self, key: T::Key<'_>) -> Option<T> {
        let Some(remove_index) = self.find_index(&T::upcast_key(key)) else {
            // The item was not found.
            return None;
        };

        self.remove_by_index(remove_index)
    }

    /// Retrieves an entry by its `key`.
    pub fn entry<'a>(&'a mut self, key: T::Key<'_>) -> Entry<'a, T> {
        let (map, dormant_map) = DormantMutRef::new(self);
        let key = T::upcast_key(key);
        {
            // index is explicitly typed to show that it has a trivial Drop impl
            // that doesn't capture anything from map.
            let index: Option<usize> = map
                .tables
                .key_to_item
                .find_index(&key, |index| map.items[index].key());
            if let Some(index) = index {
                drop(key);
                return Entry::Occupied(
                    // SAFETY: `map` is not used after this point.
                    unsafe { OccupiedEntry::new(dormant_map, index) },
                );
            }
        }
        Entry::Vacant(
            // SAFETY: `map` is not used after this point.
            unsafe { VacantEntry::new(dormant_map) },
        )
    }

    fn find<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find_index(k).map(|ix| &self.items[ix])
    }

    fn find_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.tables.key_to_item.find_index(k, |index| self.items[index].key())
    }

    pub(super) fn get_by_index(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T>>
    where
        T: IdOrdItemMut,
    {
        self.items.get_mut(index).map(RefMut::new)
    }

    pub(super) fn insert_unique_impl(
        &mut self,
        value: T,
    ) -> Result<usize, DuplicateItem<T, &T>> {
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new item, because we
        // don't want to partially insert the new item and then have to roll
        // back.
        let key = value.key();

        if let Some(index) = self
            .tables
            .key_to_item
            .find_index(&key, |index| self.items[index].key())
        {
            duplicates.insert(index);
        }

        if !duplicates.is_empty() {
            drop(key);
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.next_index();
        self.tables
            .key_to_item
            .insert(next_index, &key, |index| self.items[index].key());
        drop(key);
        self.items.insert_at_next_index(value);

        Ok(next_index)
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the table.
        self.tables.key_to_item.remove(remove_index, value.key(), |index| {
            if index == remove_index {
                value.key()
            } else {
                self.items[index].key()
            }
        });

        Some(value)
    }

    pub(super) fn replace_at_index(&mut self, index: usize, value: T) -> T {
        // We check the key before removing it, to avoid leaving the map in an
        // inconsistent state.
        let old_key =
            self.get_by_index(index).expect("index is known to be valid").key();
        if T::upcast_key(old_key) != value.key() {
            panic!(
                "must insert a value with \
                 the same key used to create the entry"
            );
        }

        // Now that we know the key is the same, we can replace the value
        // directly without needing to tweak any tables.
        self.items.replace(index, value)
    }
}

impl<T: IdOrdItem + PartialEq> PartialEq for IdBTreeMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Items are stored in sorted order, so we can just walk over both
        // iterators.
        if self.items.len() != other.items.len() {
            return false;
        }

        self.iter().zip(other.iter()).all(|(item1, item2)| {
            // Check that the items are equal.
            item1 == item2
        })
    }
}

// The Eq bound on T ensures that the IdBTreeMap forms an equivalence class.
impl<T: IdOrdItem + Eq> Eq for IdBTreeMap<T> {}

impl<'a, T: IdOrdItem> IntoIterator for &'a IdBTreeMap<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: IdOrdItemMut> IntoIterator for &'a mut IdBTreeMap<T> {
    type Item = RefMut<'a, T>;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: IdOrdItemMut> IntoIterator for IdBTreeMap<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items, self.tables)
    }
}

/// The `FromIterator` implementation for `IdBTreeMap` overwrites duplicate
/// items.
///
/// To reject duplicates, use [`IdBTreeMap::from_iter_unique`].
impl<T: IdOrdItem> FromIterator<T> for IdBTreeMap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = IdBTreeMap::new();
        for value in iter {
            map.insert_overwrite(value);
        }
        map
    }
}
