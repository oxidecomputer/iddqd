// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{tables::TriHashMapTables, IntoIter, Iter, IterMut, RefMut};
use crate::{
    errors::DuplicateItem,
    internal::ValidationError,
    support::{item_set::ItemSet, map_hash::MapHash},
    TriHashItem,
};
use derive_where::derive_where;
use hashbrown::hash_table::{Entry, VacantEntry};
use std::{borrow::Borrow, collections::BTreeSet, hash::Hash};

/// A 1:1:1 (trijective) map for three keys and a value.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in three hashmaps. This allows for efficient lookups by
/// any of the three keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone, Debug)]
pub struct TriHashMap<T: TriHashItem> {
    pub(super) items: ItemSet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: TriHashMapTables,
}

impl<T: TriHashItem> TriHashMap<T> {
    /// Creates a new, empty `TriHashMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: TriHashMapTables::new() }
    }

    /// Creates a new `TriHashMap` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity(capacity),
            tables: TriHashMapTables::with_capacity(capacity),
        }
    }

    /// Returns the currently allocated capacity of the map.
    pub fn capacity(&self) -> usize {
        // items and tables.capacity might theoretically diverge: use
        // items.capacity.
        self.items.capacity()
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
            let key3 = item.key3();

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
            let Some(ix3) = self.find3_index(&key3) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key3 index",
                    ix
                )));
            };

            if ix1 != ix || ix2 != ix || ix3 != ix {
                return Err(ValidationError::general(format!(
                    "item at index {} has inconsistent indexes: {}/{}/{}",
                    ix, ix1, ix2, ix3
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
        duplicates.extend(self.remove3(value.key3()));

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
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new item, because we
        // don't want to partially insert the new item and then have to roll
        // back.
        let (e1, e2, e3) = {
            let k1 = value.key1();
            let k2 = value.key2();
            let k3 = value.key3();

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
            let e3 = detect_dup_or_insert(
                self.tables
                    .k3_to_item
                    .entry(k3, |index| self.items[index].key3()),
                &mut duplicates,
            );
            (e1, e2, e3)
        };

        if !duplicates.is_empty() {
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.insert_at_next_index(value);
        // e1, e2 and e3 are all Some because if they were None, duplicates
        // would be non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);
        e3.unwrap().insert(next_index);

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

        let value = self
            .items
            .remove(remove_index)
            .expect("items missing key1 that was just retrieved");

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
            panic!("we just looked this item up");
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
            panic!("inconsistent indexes: key1 present, key2 absent");
        };
        let Ok(item3) =
            self.tables.k3_to_item.find_entry(&value.key3(), |index| {
                if index == remove_index {
                    value.key3()
                } else {
                    self.items[index].key3()
                }
            })
        else {
            // The item was not found.
            panic!("inconsistent indexes: key1 present, key3 absent");
        };

        item1.remove();
        item2.remove();
        item3.remove();

        Some(value)
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

        let value = self
            .items
            .remove(remove_index)
            .expect("items missing key2 that was just retrieved");

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
            panic!("inconsistent indexes: key2 present, key1 absent");
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
            panic!("we just looked this item up");
        };
        let Ok(item3) =
            self.tables.k3_to_item.find_entry(&value.key3(), |index| {
                if index == remove_index {
                    value.key3()
                } else {
                    self.items[index].key3()
                }
            })
        else {
            // The item was not found.
            panic!("inconsistent indexes: key2 present, key3 absent");
        };

        item1.remove();
        item2.remove();
        item3.remove();

        Some(value)
    }

    /// Returns true if the map contains the given `key3`.
    pub fn contains_key3<'a, Q>(&'a self, key3: &Q) -> bool
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find3_index(key3).is_some()
    }

    /// Gets a reference to the value associated with the given `key3`.
    pub fn get3<'a, Q>(&'a self, key3: &Q) -> Option<&'a T>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find3(key3)
    }

    /// Gets a mutable reference to the value associated with the given `key3`.
    ///
    /// Due to borrow checker limitations, this always accepts `K3` rather than
    /// a borrowed form of it.
    pub fn get3_mut<'a>(
        &'a mut self,
        key3: T::K3<'_>,
    ) -> Option<RefMut<'a, T>> {
        let index = self.find3_index(&T::upcast_key3(key3))?;
        let hashes = self.make_hashes(&self.items[index]);
        let item = &mut self.items[index];
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key3`.
    ///
    /// Due to borrow checker limitations, this always accepts `K1` rather than
    /// a borrowed form of it.
    pub fn remove3(&mut self, key3: T::K3<'_>) -> Option<T> {
        let Some(remove_index) = self.find3_index(&T::upcast_key3(key3)) else {
            // The item was not found.
            return None;
        };

        let value = self
            .items
            .remove(remove_index)
            .expect("items missing key3 that was just retrieved");

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
            panic!("inconsistent indexes: key3 present, key1 absent");
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
            panic!("inconsistent indexes: key3 present, key2 absent");
        };
        let Ok(item3) =
            self.tables.k3_to_item.find_entry(&value.key3(), |index| {
                if index == remove_index {
                    value.key3()
                } else {
                    self.items[index].key3()
                }
            })
        else {
            // The item was not found.
            panic!("we just looked this item up");
        };

        item1.remove();
        item2.remove();
        item3.remove();

        Some(value)
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

    fn find3<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find3_index(k).map(|ix| &self.items[ix])
    }

    fn find3_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables.k3_to_item.find_index(k, |index| self.items[index].key3())
    }

    fn make_hashes(&self, item: &T) -> [MapHash; 3] {
        self.tables.make_hashes(item)
    }
}

impl<T: TriHashItem + PartialEq> PartialEq for TriHashMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for TriHashMap is tricky because TriHashMap is
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
            let k3 = item.key3();

            // Check that the indexes are the same in the other map.
            let Some(other_ix1) = other.find1_index(&k1) else {
                return false;
            };
            let Some(other_ix2) = other.find2_index(&k2) else {
                return false;
            };
            let Some(other_ix3) = other.find3_index(&k3) else {
                return false;
            };

            if other_ix1 != other_ix2 || other_ix1 != other_ix3 {
                // All the keys were present but they didn't point to the same
                // item.
                return false;
            }

            // Check that the other map's item is the same as this map's
            // item. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix1, other_ix2 and other_ix3 are
            // Some, we know that it is valid and points to the expected item.
            let other_item = &other.items[other_ix1];
            if item != other_item {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the TriHashMap forms an equivalence class.
impl<T: TriHashItem + Eq> Eq for TriHashMap<T> {}

fn detect_dup_or_insert<'a>(
    item: Entry<'a, usize>,
    duplicates: &mut BTreeSet<usize>,
) -> Option<VacantEntry<'a, usize>> {
    match item {
        Entry::Vacant(slot) => Some(slot),
        Entry::Occupied(slot) => {
            duplicates.insert(*slot.get());
            None
        }
    }
}

impl<'a, T: TriHashItem> IntoIterator for &'a TriHashMap<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: TriHashItem> IntoIterator for &'a mut TriHashMap<T> {
    type Item = RefMut<'a, T>;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: TriHashItem> IntoIterator for TriHashMap<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items)
    }
}

/// The `FromIterator` implementation for `TriHashMap` overwrites duplicate
/// items.
impl<T: TriHashItem> FromIterator<T> for TriHashMap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = TriHashMap::new();
        for item in iter {
            map.insert_overwrite(item);
        }
        map
    }
}
