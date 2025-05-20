// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{
    entry::{EntryHash, EntryIndex},
    tables::BiHashMapTables,
    Entry, IntoIter, Iter, IterMut, OccupiedEntry, RefMut, VacantEntry,
};
use crate::{
    errors::DuplicateItem,
    internal::ValidationError,
    support::{borrow::DormantMutRef, hash_table::MapHash, item_set::ItemSet},
    BiHashItem,
};
use derive_where::derive_where;
use hashbrown::hash_table;
use std::{borrow::Borrow, collections::BTreeSet, hash::Hash};

/// A 1:1 (bijective) map for two keys and a value.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in two hashmaps. This allows for efficient lookups by
/// either of the two keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone, Debug)]
pub struct BiHashMap<T: BiHashItem> {
    pub(super) items: ItemSet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: BiHashMapTables,
}

impl<T: BiHashItem> BiHashMap<T> {
    /// Creates a new, empty `BiHashMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: BiHashMapTables::new() }
    }

    /// Creates a new `BiHashMap` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity(capacity),
            tables: BiHashMapTables::with_capacity(capacity),
        }
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
        Iter::new(&self.items)
    }

    /// Iterates over the items in the map, allowing for mutation.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(&self.tables, &mut self.items)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[doc(hidden)]
    pub fn validate(
        &self,
        compactness: crate::internal::ValidateCompact,
    ) -> Result<(), ValidationError>
    where
        T: std::fmt::Debug,
    {
        self.tables.validate(self.items.len(), compactness)?;

        // Check that the indexes are all correct.
        for (&ix, item) in self.items.iter() {
            let key1 = item.key1();
            let key2 = item.key2();

            let Some(ix1) = self.find1_index(&key1) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key1 index",
                    ix
                )));
            };
            let Some(ix2) = self.find2_index(&key2) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key2 index",
                    ix
                )));
            };

            if ix1 != ix || ix2 != ix {
                return Err(ValidationError::general(format!(
                    "item at index {} has inconsistent indexes: {}/{}",
                    ix, ix1, ix2
                )));
            }
        }

        Ok(())
    }

    /// Inserts a value into the map, removing any conflicting items and
    /// returning a list of those items.
    pub fn insert_overwrite(&mut self, value: T) -> Vec<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove items corresponding to keys that are already in the map.
        // 2. Add the item to the map.

        let mut duplicates = Vec::new();
        duplicates.extend(self.remove1(value.key1()));
        duplicates.extend(self.remove2(value.key2()));

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicates
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

    /// Returns true if the map contains the given `key1`.
    pub fn contains_key1<'a, Q>(&'a self, key1: &Q) -> bool
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find1_index(key1).is_some()
    }

    /// Gets a reference to the value associated with the given `key1`.
    pub fn get1<'a, Q>(&'a self, key1: &Q) -> Option<&'a T>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find1(key1)
    }

    /// Gets a mutable reference to the value associated with the given `key1`.
    ///
    /// Due to borrow checker limitations, this always accepts `K1` rather than
    /// a borrowed form of it.
    pub fn get1_mut<'a>(
        &'a mut self,
        key1: T::K1<'_>,
    ) -> Option<RefMut<'a, T>> {
        let index = self.find1_index(&T::upcast_key1(key1))?;
        let hashes = self.make_hashes(&self.items[index]);
        let item = &mut self.items[index];
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key1`.
    ///
    /// Due to borrow checker limitations, this always accepts `K1` rather than
    /// a borrowed form of it.
    pub fn remove1(&mut self, key1: T::K1<'_>) -> Option<T> {
        let Some(remove_index) = self.find1_index(&T::upcast_key1(key1)) else {
            // The item was not found.
            return None;
        };

        self.remove_by_index(remove_index)
    }

    /// Retrieves an entry by its `key1`.
    pub fn entry1<'a>(&'a mut self, key1: T::K1<'_>) -> Entry<'a, T> {
        let (map, dormant_map) = DormantMutRef::new(self);
        let key = T::upcast_key1(key1);
        {
            // index is explicitly typed to show that it has a trivial Drop impl
            // that doesn't capture anything from map.
            let index: Option<usize> = map
                .tables
                .k1_to_item
                .find_index(&key, |index| map.items[index].key1());
            if let Some(index) = index {
                drop(key);
                return Entry::Occupied(
                    // SAFETY: `map` is not used after this point.
                    unsafe {
                        OccupiedEntry::new(dormant_map, EntryIndex::Key1(index))
                    },
                );
            }
        }
        let hash = map.make_key1_hash(&key);
        Entry::Vacant(
            // SAFETY: `map` is not used after this point.
            unsafe { VacantEntry::new(dormant_map, hash) },
        )
    }

    /// Returns true if the map contains the given `key2`.
    pub fn contains_key2<'a, Q>(&'a self, key2: &Q) -> bool
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find2_index(key2).is_some()
    }

    /// Gets a reference to the value associated with the given `key2`.
    pub fn get2<'a, Q>(&'a self, key2: &Q) -> Option<&'a T>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find2(key2)
    }

    /// Gets a mutable reference to the value associated with the given `key2`.
    ///
    /// Due to borrow checker limitations, this always accepts `K2` rather than
    /// a borrowed form of it.
    pub fn get2_mut<'a>(
        &'a mut self,
        key2: T::K2<'_>,
    ) -> Option<RefMut<'a, T>> {
        let index = self.find2_index(&T::upcast_key2(key2))?;
        let hashes = self.make_hashes(&self.items[index]);
        let item = &mut self.items[index];
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key2`.
    ///
    /// Due to borrow checker limitations, this always accepts `K1` rather than
    /// a borrowed form of it.
    pub fn remove2(&mut self, key2: T::K2<'_>) -> Option<T> {
        let Some(remove_index) = self.find2_index(&T::upcast_key2(key2)) else {
            // The item was not found.
            return None;
        };

        self.remove_by_index(remove_index)
    }

    /// Retrieves an entry by its `key2`.
    pub fn entry2<'a>(&'a mut self, key2: T::K2<'_>) -> Entry<'a, T> {
        let (map, dormant_map) = DormantMutRef::new(self);
        let key = T::upcast_key2(key2);
        {
            // index is explicitly typed to show that it has a trivial Drop impl
            // that doesn't capture anything from map.
            let index: Option<usize> = map
                .tables
                .k2_to_item
                .find_index(&key, |index| map.items[index].key2());
            if let Some(index) = index {
                drop(key);
                return Entry::Occupied(
                    // SAFETY: `map` is not used after this point.
                    unsafe {
                        OccupiedEntry::new(dormant_map, EntryIndex::Key2(index))
                    },
                );
            }
        }
        let hash = map.make_key2_hash(&key);
        Entry::Vacant(
            // SAFETY: `map` is not used after this point.
            unsafe { VacantEntry::new(dormant_map, hash) },
        )
    }

    fn find1<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find1_index(k).map(|ix| &self.items[ix])
    }

    fn find1_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables.k1_to_item.find_index(k, |index| self.items[index].key1())
    }

    fn find2<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find2_index(k).map(|ix| &self.items[ix])
    }

    fn find2_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables.k2_to_item.find_index(k, |index| self.items[index].key2())
    }

    pub(super) fn get_by_index(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T>> {
        let borrowed = self.items.get_mut(index)?;
        let hashes = self.tables.make_hashes(borrowed);
        let item = &mut self.items[index];
        Some(RefMut::new(hashes, item))
    }

    pub(super) fn insert_unique_impl(
        &mut self,
        value: T,
    ) -> Result<usize, DuplicateItem<T, &T>> {
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new item, because we
        // don't want to partially insert the new item and then have to roll
        // back.
        let (e1, e2) = {
            let k1 = value.key1();
            let k2 = value.key2();

            let e1 = detect_dup_or_insert(
                self.tables
                    .k1_to_item
                    .entry(k1, |index| self.items[index].key1()),
                &mut duplicates,
            );
            let e2 = detect_dup_or_insert(
                self.tables
                    .k2_to_item
                    .entry(k2, |index| self.items[index].key2()),
                &mut duplicates,
            );
            (e1, e2)
        };

        if !duplicates.is_empty() {
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.insert_at_next_index(value);
        // e1 and e2 are all Some because if they were None, duplicates
        // would be non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);

        Ok(next_index)
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the tables.
        let Ok(item1) =
            self.tables.k1_to_item.find_entry(&value.key1(), |index| {
                if index == remove_index {
                    value.key1()
                } else {
                    self.items[index].key1()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k1_to_item");
        };
        let Ok(item2) =
            self.tables.k2_to_item.find_entry(&value.key2(), |index| {
                if index == remove_index {
                    value.key2()
                } else {
                    self.items[index].key2()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k2_to_item")
        };

        item1.remove();
        item2.remove();

        Some(value)
    }

    pub(super) fn replace_at_index(
        &mut self,
        index: EntryIndex,
        value: T,
    ) -> Vec<T> {
        {
            let key1 = value.key1();
            let key2 = value.key2();

            // We check the key before removing it, to avoid leaving the map in an
            // inconsistent state.
            let old_item = self
                .get_by_index(index.index())
                .expect("index is known to be valid");
            match index {
                EntryIndex::Key1(_) => {
                    if key1 != old_item.key1() {
                        panic!("key1 does not match item at index");
                    }
                }
                EntryIndex::Key2(_) => {
                    if key2 != old_item.key2() {
                        panic!("key2 does not match item at index");
                    }
                }
            }
        }

        // Now that we know the key is the same, remove the value at index, but
        // also any other items that conflict.
        let mut old_items = Vec::new();
        match index {
            EntryIndex::Key1(_) => {
                // Also remove by key2 if found.
                old_items.extend(self.remove2(value.key2()));
            }
            EntryIndex::Key2(_) => {
                // Also remove by key1 if found.
                old_items.extend(self.remove1(value.key1()));
            }
        }

        // Remove the item at the provided index after removing other items, so
        // that `self.remove1` and `self.remove2` don't see an inconsistent
        // state.
        old_items.push(self.items.replace(index.index(), value));
        old_items
    }

    fn make_key1_hash(&self, key: &T::K1<'_>) -> EntryHash {
        EntryHash::Key1(self.tables.k1_to_item.compute_hash(key))
    }

    fn make_key2_hash(&self, key: &T::K2<'_>) -> EntryHash {
        EntryHash::Key2(self.tables.k2_to_item.compute_hash(key))
    }

    fn make_hashes(&self, item: &T) -> [MapHash; 2] {
        self.tables.make_hashes(item)
    }
}

impl<T: BiHashItem + PartialEq> PartialEq for BiHashMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for BiHashMap is tricky because BiHashMap is
        // not semantically like an IndexMap: two maps are equivalent even if
        // their items are in a different order. In other words, any permutation
        // of items is equivalent.
        //
        // We also can't sort the items because they're not necessarily Ord.
        //
        // So we write a custom equality check that checks that each key in one
        // map points to the same item as in the other map.

        if self.items.len() != other.items.len() {
            return false;
        }

        // Walk over all the items in the first map and check that they point to
        // the same item in the second map.
        for item in self.items.values() {
            let k1 = item.key1();
            let k2 = item.key2();

            // Check that the indexes are the same in the other map.
            let Some(other_ix1) = other.find1_index(&k1) else {
                return false;
            };
            let Some(other_ix2) = other.find2_index(&k2) else {
                return false;
            };

            if other_ix1 != other_ix2 {
                // All the keys were present but they didn't point to the same
                // item.
                return false;
            }

            // Check that the other map's item is the same as this map's
            // item. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix1 and other_ix2 are
            // Some, we know that it is valid and points to the expected item.
            let other_item = &other.items[other_ix1];
            if item != other_item {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the BiHashMap forms an equivalence class.
impl<T: BiHashItem + Eq> Eq for BiHashMap<T> {}

fn detect_dup_or_insert<'a>(
    item: hash_table::Entry<'a, usize>,
    duplicates: &mut BTreeSet<usize>,
) -> Option<hash_table::VacantEntry<'a, usize>> {
    match item {
        hash_table::Entry::Vacant(slot) => Some(slot),
        hash_table::Entry::Occupied(slot) => {
            duplicates.insert(*slot.get());
            None
        }
    }
}

impl<'a, T: BiHashItem> IntoIterator for &'a BiHashMap<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: BiHashItem> IntoIterator for &'a mut BiHashMap<T> {
    type Item = RefMut<'a, T>;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: BiHashItem> IntoIterator for BiHashMap<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items)
    }
}

/// The `FromIterator` implementation for `BiHashMap` overwrites duplicate
/// items.
impl<T: BiHashItem> FromIterator<T> for BiHashMap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = BiHashMap::new();
        for item in iter {
            map.insert_overwrite(item);
        }
        map
    }
}
