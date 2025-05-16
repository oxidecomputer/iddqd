// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{
    tables::IdBTreeMapTables, IdBTreeMapEntry, IdBTreeMapEntryMut, IntoIter,
    Iter, IterMut, RefMut,
};
use crate::{errors::DuplicateEntry, support::entry_set::EntrySet};
use derive_where::derive_where;
use std::{borrow::Borrow, collections::BTreeSet};

/// An ordered map where the keys are part of the values, based on a B-Tree.
///
/// The storage mechanism is a fast hash table of integer indexes to entries,
/// with these indexes stored in three b-tree maps. This allows for efficient
/// lookups by any of the three keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone, Debug)]
pub struct IdBTreeMap<T: IdBTreeMapEntry> {
    pub(super) entries: EntrySet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `entries`, and are a 1:1 mapping.
    tables: IdBTreeMapTables,
}

impl<T: IdBTreeMapEntry> IdBTreeMap<T> {
    /// Creates a new, empty `IdBTreeMap`.
    #[inline]
    pub fn new() -> Self {
        Self { entries: EntrySet::default(), tables: IdBTreeMapTables::new() }
    }

    /// Returns true if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Iterates over the entries in the map.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.entries, &self.tables)
    }

    /// Iterates over the entries in the map, allowing for mutation.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T>
    where
        T: IdBTreeMapEntryMut,
    {
        IterMut::new(&mut self.entries, &self.tables)
    }

    /// Consumes self, returning an iterator over the entries in the map.
    #[inline]
    pub fn into_iter(self) -> IntoIter<T> {
        IntoIter::new(self.entries, self.tables)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[cfg(test)]
    pub(crate) fn validate(&self) -> anyhow::Result<()>
    where
        T: std::fmt::Debug,
    {
        use anyhow::Context;

        self.tables.validate(self.entries.len())?;

        // Check that the indexes are all correct.
        for (&ix, entry) in self.entries.iter() {
            let key = entry.key();

            let ix1 = self.find_index(&key).with_context(|| {
                format!("entry at index {ix} has no key index")
            })?;

            if ix1 != ix {
                return Err(anyhow::anyhow!(
                    "entry at index {ix} has mismatched indexes: {} != {}",
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
    ) -> Result<(), DuplicateEntry<T, &T>> {
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new entry, because we
        // don't want to partially insert the new entry and then have to roll
        // back.
        let key = value.key();

        if let Some(index) = self
            .tables
            .key_to_entry
            .find_index(&key, |index| self.entries[index].key())
        {
            duplicates.insert(index);
        }

        if !duplicates.is_empty() {
            drop(key);
            return Err(DuplicateEntry::new(
                value,
                duplicates.iter().map(|ix| &self.entries[*ix]).collect(),
            ));
        }

        let next_index = self.entries.next_index();
        self.tables
            .key_to_entry
            .insert(next_index, &key, |index| self.entries[index].key());
        drop(key);
        self.entries.insert(value);

        Ok(())
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

    /// Gets a mutable reference to the value associated with the given `key`.
    ///
    /// Due to borrow checker limitations, this requires that `Key` have an owned form.
    pub fn get_mut<'a>(&'a mut self, key: T::Key<'_>) -> Option<RefMut<'a, T>>
    where
        T: IdBTreeMapEntryMut,
    {
        let index = self.find_index(&T::upcast_key(key))?;
        let entry = &mut self.entries[index];
        Some(RefMut::new(entry))
    }

    fn find<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find_index(k).map(|ix| &self.entries[ix])
    }

    fn find_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.tables
            .key_to_entry
            .find_index(k, |index| self.entries[index].key())
    }
}

impl<T: IdBTreeMapEntry + PartialEq> PartialEq for IdBTreeMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Entries are stored in sorted order, so we can just walk over both
        // iterators.
        if self.entries.len() != other.entries.len() {
            return false;
        }

        self.iter().zip(other.iter()).all(|(entry1, entry2)| {
            // Check that the entries are equal.
            entry1 == entry2
        })
    }
}

// The Eq bound on T ensures that the IdBTreeMap forms an equivalence class.
impl<T: IdBTreeMapEntry + Eq> Eq for IdBTreeMap<T> {}

impl<'a, T: IdBTreeMapEntry> IntoIterator for &'a IdBTreeMap<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: IdBTreeMapEntryMut> IntoIterator for &'a mut IdBTreeMap<T> {
    type Item = RefMut<'a, T>;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: IdBTreeMapEntryMut> IntoIterator for IdBTreeMap<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        assert_eq_props, assert_iter_eq, assert_ne_props,
        test_entry_permutation_strategy, TestEntry,
    };
    use proptest::prelude::*;
    use test_strategy::{proptest, Arbitrary};

    #[test]
    fn test_insert_unique() {
        let mut map = IdBTreeMap::<TestEntry>::new();

        // Add an element.
        let v1 = TestEntry {
            key1: 20,
            key2: 'a',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        map.insert_unique(v1.clone()).unwrap();

        // Add an exact duplicate, which should error out.
        let error = map.insert_unique(v1.clone()).unwrap_err();
        assert_eq!(error.new_entry(), &v1);
        assert_eq!(error.duplicates(), vec![&v1]);

        // Add a duplicate against just key1, which should error out.
        let v2 = TestEntry {
            key1: 20,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_unique(v2.clone()).unwrap_err();
        assert_eq!(error.new_entry(), &v2);
        assert_eq!(error.duplicates(), vec![&v1]);

        // Add a duplicate against key2. IdBTreeMap only uses key1 here, so this
        // should be allowed.
        let v3 = TestEntry {
            key1: 5,
            key2: 'a',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        map.insert_unique(v3.clone()).unwrap();

        // Add a duplicate against key1, which should error out.
        let v4 = TestEntry {
            key1: 5,
            key2: 'b',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_unique(v4.clone()).unwrap_err();
        assert_eq!(error.new_entry(), &v4);

        // Iterate over the entries mutably. This ensures that miri detects
        // unsafety if it exists.
        let entries: Vec<RefMut<_>> = map.iter_mut().collect();
        let e1 = &*entries[0];
        assert_eq!(*e1, v3);

        let e2 = &*entries[1];
        assert_eq!(*e2, v1);
    }

    /// Represents a naive version of `IdBTreeMap` that doesn't have any indexes
    /// and does linear scans.
    #[derive(Debug)]
    struct NaiveMap {
        entries: Vec<TestEntry>,
    }

    impl NaiveMap {
        fn new() -> Self {
            Self { entries: Vec::new() }
        }

        fn insert_unique(
            &mut self,
            entry: TestEntry,
        ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>> {
            // Cannot store the duplicates directly here because of borrow
            // checker issues. Instead, we store indexes and then map them to
            // entries.
            let indexes =
                self.entries
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        if e.key1 == entry.key1 {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

            if indexes.is_empty() {
                self.entries.push(entry);
                Ok(())
            } else {
                Err(DuplicateEntry::new(
                    entry,
                    indexes.iter().map(|&i| &self.entries[i]).collect(),
                ))
            }
        }
    }

    #[derive(Debug, Arbitrary)]
    enum Operation {
        // Make inserts a bit more common to try and fill up the map.
        #[weight(3)]
        Insert(TestEntry),
        Get(u8),
    }

    // Miri is quite slow, so run fewer operations.
    #[cfg(miri)]
    const OP_LEN: usize = 64;
    #[cfg(miri)]
    const PERMUTATION_LEN: usize = 16;
    #[cfg(not(miri))]
    const OP_LEN: usize = 1024;
    #[cfg(not(miri))]
    const PERMUTATION_LEN: usize = 256;

    #[proptest(cases = 16)]
    fn proptest_ops(
        #[strategy(prop::collection::vec(any::<Operation>(), 0..OP_LEN))]
        ops: Vec<Operation>,
    ) {
        let mut map = IdBTreeMap::<TestEntry>::new();
        let mut naive_map = NaiveMap::new();

        // Now perform the operations on both maps.
        for op in ops {
            match op {
                Operation::Insert(entry) => {
                    let map_res = map.insert_unique(entry.clone());
                    let naive_res = naive_map.insert_unique(entry.clone());

                    assert_eq!(map_res.is_ok(), naive_res.is_ok());
                    if let Err(map_err) = map_res {
                        let naive_err = naive_res.unwrap_err();
                        assert_eq!(map_err.new_entry(), naive_err.new_entry());
                        assert_eq!(
                            map_err.duplicates(),
                            naive_err.duplicates()
                        );
                    }

                    map.validate().expect("map should be valid");
                }
                Operation::Get(key1) => {
                    let map_res = map.get(&key1);
                    let naive_res =
                        naive_map.entries.iter().find(|e| e.key1 == key1);

                    assert_eq!(map_res, naive_res);
                }
            }

            // Check that the iterators work correctly.
            let mut naive_entries =
                naive_map.entries.iter().collect::<Vec<_>>();
            naive_entries.sort_by_key(|e| *e.key());

            assert_iter_eq(map.clone(), naive_entries);
        }
    }

    #[proptest(cases = 64)]
    fn proptest_permutation_eq(
        #[strategy(test_entry_permutation_strategy::<IdBTreeMap<TestEntry>>(0..PERMUTATION_LEN))]
        entries: (Vec<TestEntry>, Vec<TestEntry>),
    ) {
        let (entries1, entries2) = entries;
        let mut map1 = IdBTreeMap::<TestEntry>::new();
        let mut map2 = IdBTreeMap::<TestEntry>::new();

        for entry in entries1 {
            map1.insert_unique(entry.clone()).unwrap();
        }
        for entry in entries2 {
            map2.insert_unique(entry.clone()).unwrap();
        }

        assert_eq_props(map1, map2);
    }

    // Test various conditions for non-equality.
    #[test]
    fn test_permutation_eq_examples() {
        let mut map1 = IdBTreeMap::<TestEntry>::new();
        let mut map2 = IdBTreeMap::<TestEntry>::new();

        // Two empty maps are equal.
        assert_eq!(map1, map2);

        // Insert a single entry into one map.
        let entry = TestEntry {
            key1: 0,
            key2: 'a',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        map1.insert_unique(entry.clone()).unwrap();

        // The maps are not equal.
        assert_ne_props(&map1, &map2);

        // Insert the same entry into the other map.
        map2.insert_unique(entry.clone()).unwrap();

        // The maps are now equal.
        assert_eq_props(&map1, &map2);

        {
            // Insert an entry with the same key2 and key3 but a different
            // key1.
            let mut map1 = map1.clone();
            map1.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_unique(TestEntry {
                key1: 2,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);
        }

        {
            // Insert an entry with the same key1 and key3 but a different
            // key2.
            let mut map1 = map1.clone();
            map1.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_unique(TestEntry {
                key1: 1,
                key2: 'c',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);
        }

        {
            // Insert an entry with the same key1 and key2 but a different
            // key3.
            let mut map1 = map1.clone();
            map1.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "z".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);
        }

        {
            // Insert an entry where all the keys are the same, but the value is
            // different.
            let mut map1 = map1.clone();
            map1.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "w".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_unique(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "x".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);
        }
    }
}
