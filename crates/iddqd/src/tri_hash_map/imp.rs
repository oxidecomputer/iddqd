// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{Iter, IterMut, RefMut};
use crate::{
    support::{
        entry_set::EntrySet,
        hash_table::{MapHash, MapHashTable},
    },
    TriHashMapEntry,
};
use derive_where::derive_where;
use hashbrown::hash_table::{Entry, VacantEntry};
use std::{borrow::Borrow, collections::BTreeSet, fmt, hash::Hash};

/// An append-only 1:1:1 (trijective) map for three keys and a value.
///
/// The storage mechanism is a vector of entries, with indexes into that vector
/// stored in three hashmaps. This allows for efficient lookups by any of the
/// three keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone, Debug)]
pub struct TriHashMap<T: TriHashMapEntry> {
    pub(super) entries: EntrySet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `entries`, and are a 1:1 mapping.
    tables: TriHashMapTables,
}

impl<T: TriHashMapEntry> TriHashMap<T> {
    #[inline]
    pub fn new() -> Self {
        Self { entries: EntrySet::default(), tables: TriHashMapTables::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: EntrySet::with_capacity(capacity),
            tables: TriHashMapTables::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.entries)
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(&self.tables, &mut self.entries)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[cfg(test)]
    pub(super) fn validate(&self) -> anyhow::Result<()>
    where
        T: fmt::Debug,
    {
        use anyhow::Context;

        self.tables.validate(self.entries.len())?;

        // Check that the indexes are all correct.
        for (&ix, entry) in self.entries.iter() {
            let key1 = entry.key1();
            let key2 = entry.key2();
            let key3 = entry.key3();

            let ix1 = self.find1_index(&key1).with_context(|| {
                format!("entry at index {ix} has no key1 index")
            })?;
            let ix2 = self.find2_index(&key2).with_context(|| {
                format!("entry at index {ix} has no key2 index")
            })?;
            let ix3 = self.find3_index(&key3).with_context(|| {
                format!("entry at index {ix} has no key3 index")
            })?;

            if ix1 != ix || ix2 != ix || ix3 != ix {
                return Err(anyhow::anyhow!(
                    "entry at index {} has mismatched indexes: ix1: {}, ix2: {}, ix3: {}",
                    ix,
                    ix1,
                    ix2,
                    ix3
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
        let (e1, e2, e3) = {
            let k1 = value.key1();
            let k2 = value.key2();
            let k3 = value.key3();

            let e1 = detect_dup_or_insert(
                self.tables
                    .k1_to_entry
                    .entry(k1, |index| self.entries[index].key1()),
                &mut duplicates,
            );
            let e2 = detect_dup_or_insert(
                self.tables
                    .k2_to_entry
                    .entry(k2, |index| self.entries[index].key2()),
                &mut duplicates,
            );
            let e3 = detect_dup_or_insert(
                self.tables
                    .k3_to_entry
                    .entry(k3, |index| self.entries[index].key3()),
                &mut duplicates,
            );
            (e1, e2, e3)
        };

        if !duplicates.is_empty() {
            return Err(DuplicateEntry {
                new: value,
                duplicates: duplicates
                    .iter()
                    .map(|ix| &self.entries[*ix])
                    .collect(),
            });
        }

        let next_index = self.entries.len();
        // e1, e2 and e3 are all Some because if they were None, duplicates
        // would be non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);
        e3.unwrap().insert(next_index);
        self.entries.insert(value);

        Ok(())
    }

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
        let hashes = self.make_hashes(&self.entries[index]);
        let entry = &mut self.entries[index];
        Some(RefMut::new(hashes, entry))
    }

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
        let hashes = self.make_hashes(&self.entries[index]);
        let entry = &mut self.entries[index];
        Some(RefMut::new(hashes, entry))
    }

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
        let hashes = self.make_hashes(&self.entries[index]);
        let entry = &mut self.entries[index];
        Some(RefMut::new(hashes, entry))
    }

    fn find1<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find1_index(k).map(|ix| &self.entries[ix])
    }

    fn find1_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables
            .k1_to_entry
            .find_index(k, |index| self.entries[index].key1())
    }

    fn find2<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find2_index(k).map(|ix| &self.entries[ix])
    }

    fn find2_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables
            .k2_to_entry
            .find_index(k, |index| self.entries[index].key2())
    }

    fn find3<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find3_index(k).map(|ix| &self.entries[ix])
    }

    fn find3_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.tables
            .k3_to_entry
            .find_index(k, |index| self.entries[index].key3())
    }

    fn make_hashes(&self, item: &T) -> [MapHash; 3] {
        self.tables.make_hashes(item)
    }
}

impl<T: TriHashMapEntry + PartialEq> PartialEq for TriHashMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for TriHashMap is tricky because TriHashMap is
        // not semantically like an IndexMap: two maps are equivalent even if
        // their entries are in a different order. In other words, any
        // permutation of entries is equivalent.
        //
        // We also can't sort the entries because they're not necessarily Ord.
        //
        // So we write a custom equality check that checks that each key in one
        // map points to the same entry as in the other map.

        if self.entries.len() != other.entries.len() {
            return false;
        }

        // Walk over all the entries in the first map and check that they point
        // to the same entry in the second map.
        for entry in self.entries.values() {
            let k1 = entry.key1();
            let k2 = entry.key2();
            let k3 = entry.key3();

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
                // entry.
                return false;
            }

            // Check that the other map's entry is the same as this map's
            // entry. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix1, other_ix2 and other_ix3 are
            // Some, we know that it is valid and points to the expected entry.
            let other_entry = &other.entries[other_ix1];
            if entry != other_entry {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the TriHashMap forms an equivalence class.
impl<T: TriHashMapEntry + Eq> Eq for TriHashMap<T> {}

fn detect_dup_or_insert<'a>(
    entry: Entry<'a, usize>,
    duplicates: &mut BTreeSet<usize>,
) -> Option<VacantEntry<'a, usize>> {
    match entry {
        Entry::Vacant(slot) => Some(slot),
        Entry::Occupied(slot) => {
            duplicates.insert(*slot.get());
            None
        }
    }
}

#[derive(Debug)]
pub struct DuplicateEntry<T: TriHashMapEntry, D: TriHashMapEntry = T> {
    new: T,
    duplicates: Vec<D>,
}

impl<T: TriHashMapEntry, D: TriHashMapEntry> DuplicateEntry<T, D> {
    /// Returns the new entry that was attempted to be inserted.
    #[inline]
    pub fn new_entry(&self) -> &T {
        &self.new
    }

    /// Returns the list of entries that conflict with the new entry.
    #[inline]
    pub fn duplicates(&self) -> &[D] {
        &self.duplicates
    }

    /// Converts self into its constituent parts.
    pub fn into_parts(self) -> (T, Vec<D>) {
        (self.new, self.duplicates)
    }
}

impl<T: TriHashMapEntry + Clone> DuplicateEntry<T, &T> {
    /// Converts self to an owned `DuplicateEntry` by cloning the list of
    /// duplicates.
    ///
    /// If `T` is `'static`, the owned form is suitable for conversion to
    /// `Box<dyn std::error::Error>`, `anyhow::Error`, and so on.
    pub fn into_owned(self) -> DuplicateEntry<T> {
        DuplicateEntry {
            new: self.new,
            duplicates: self.duplicates.into_iter().cloned().collect(),
        }
    }
}

impl<T: TriHashMapEntry + fmt::Debug, D: TriHashMapEntry + fmt::Debug>
    fmt::Display for DuplicateEntry<T, D>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "new entry: {:?} conflicts with existing: {:?}",
            self.new, self.duplicates
        )
    }
}

impl<T: TriHashMapEntry + fmt::Debug, D: TriHashMapEntry + fmt::Debug>
    std::error::Error for DuplicateEntry<T, D>
{
}

#[derive(Clone, Debug, Default)]
pub(super) struct TriHashMapTables {
    k1_to_entry: MapHashTable,
    k2_to_entry: MapHashTable,
    k3_to_entry: MapHashTable,
}

impl TriHashMapTables {
    fn new() -> Self {
        Self::default()
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            k1_to_entry: MapHashTable::with_capacity(capacity),
            k2_to_entry: MapHashTable::with_capacity(capacity),
            k3_to_entry: MapHashTable::with_capacity(capacity),
        }
    }

    #[cfg(test)]
    fn validate(&self, expected_len: usize) -> anyhow::Result<()> {
        // Check that all the maps are of the right size.

        use anyhow::Context;
        self.k1_to_entry
            .validate(expected_len)
            .context("k1_to_entry failed validation")?;
        self.k2_to_entry
            .validate(expected_len)
            .context("k2_to_entry failed validation")?;
        self.k3_to_entry
            .validate(expected_len)
            .context("k3_to_entry failed validation")?;

        Ok(())
    }

    pub(super) fn make_hashes<T: TriHashMapEntry>(
        &self,
        item: &T,
    ) -> [MapHash; 3] {
        let k1 = item.key1();
        let k2 = item.key2();
        let k3 = item.key3();

        let h1 = self.k1_to_entry.compute_hash(k1);
        let h2 = self.k2_to_entry.compute_hash(k2);
        let h3 = self.k3_to_entry.compute_hash(k3);

        [h1, h2, h3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tri_hash_map::test_utils::TestEntry;
    use prop::sample::SizeRange;
    use proptest::prelude::*;
    use test_strategy::{proptest, Arbitrary};

    #[test]
    fn test_insert_entry_no_dups() {
        let mut map = TriHashMap::<TestEntry>::new();

        // Add an element.
        let v1 = TestEntry {
            key1: 0,
            key2: 'a',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        map.insert_unique(v1.clone()).unwrap();

        // Add an exact duplicate, which should error out.
        let error = map.insert_unique(v1.clone()).unwrap_err();
        assert_eq!(&error.new, &v1);
        assert_eq!(error.duplicates, vec![&v1]);

        // Add a duplicate against just key1, which should error out.
        let v2 = TestEntry {
            key1: 0,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_unique(v2.clone()).unwrap_err();
        assert_eq!(&error.new, &v2);
        assert_eq!(error.duplicates, vec![&v1]);

        // Add a duplicate against just key2, which should error out.
        let v3 = TestEntry {
            key1: 1,
            key2: 'a',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_unique(v3.clone()).unwrap_err();
        assert_eq!(&error.new, &v3);

        // Add a duplicate against just key3, which should error out.
        let v4 = TestEntry {
            key1: 1,
            key2: 'b',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_unique(v4.clone()).unwrap_err();
        assert_eq!(&error.new, &v4);

        // Add an entry that doesn't have any conflicts.
        let v5 = TestEntry {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        map.insert_unique(v5.clone()).unwrap();
    }

    /// Represents a naive version of `TriMap` that doesn't have any indexes
    /// and does linear scans.
    #[derive(Debug)]
    struct NaiveTriMap {
        entries: Vec<TestEntry>,
    }

    impl NaiveTriMap {
        fn new() -> Self {
            Self { entries: Vec::new() }
        }

        fn insert_entry_no_dups(
            &mut self,
            entry: TestEntry,
        ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>> {
            // Cannot store the duplicates directly here because of borrow
            // checker issues. Instead, we store indexes and then map them to
            // entries.
            let indexes = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    if e.key1 == entry.key1
                        || e.key2 == entry.key2
                        || e.key3 == entry.key3
                    {
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
                Err(DuplicateEntry {
                    new: entry,
                    duplicates: indexes
                        .iter()
                        .map(|&i| &self.entries[i])
                        .collect(),
                })
            }
        }
    }

    #[derive(Debug, Arbitrary)]
    enum Operation {
        // Make inserts a bit more common to try and fill up the map.
        #[weight(3)]
        Insert(TestEntry),
        Get1(u8),
        Get2(char),
        Get3(String),
    }

    #[proptest(cases = 16)]
    fn proptest_ops(
        #[strategy(prop::collection::vec(any::<Operation>(), 0..1024))]
        ops: Vec<Operation>,
    ) {
        let mut map = TriHashMap::<TestEntry>::new();
        let mut naive_map = NaiveTriMap::new();

        // Now perform the operations on both maps.
        for op in ops {
            match op {
                Operation::Insert(entry) => {
                    let map_res = map.insert_unique(entry.clone());
                    let naive_res =
                        naive_map.insert_entry_no_dups(entry.clone());

                    assert_eq!(map_res.is_ok(), naive_res.is_ok());
                    if let Err(map_err) = map_res {
                        let naive_err = naive_res.unwrap_err();
                        assert_eq!(map_err.new, naive_err.new);
                        assert_eq!(map_err.duplicates, naive_err.duplicates);
                    }

                    map.validate().expect("map should be valid");
                }
                Operation::Get1(key1) => {
                    let map_res = map.get1(&key1);
                    let naive_res =
                        naive_map.entries.iter().find(|e| e.key1 == key1);

                    assert_eq!(map_res, naive_res);
                }
                Operation::Get2(key2) => {
                    let map_res = map.get2(&key2);
                    let naive_res =
                        naive_map.entries.iter().find(|e| e.key2 == key2);

                    assert_eq!(map_res, naive_res);
                }
                Operation::Get3(key3) => {
                    let map_res = map.get3(key3.as_str());
                    let naive_res =
                        naive_map.entries.iter().find(|e| e.key3 == key3);

                    assert_eq!(map_res, naive_res);
                }
            }
        }
    }

    #[proptest(cases = 64)]
    fn proptest_permutation_eq(
        #[strategy(test_entry_permutation_strategy(0..256))] entries: (
            Vec<TestEntry>,
            Vec<TestEntry>,
        ),
    ) {
        let (entries1, entries2) = entries;
        let mut map1 = TriHashMap::<TestEntry>::new();
        let mut map2 = TriHashMap::<TestEntry>::new();

        for entry in entries1 {
            map1.insert_unique(entry.clone()).unwrap();
        }
        for entry in entries2 {
            map2.insert_unique(entry.clone()).unwrap();
        }

        assert_eq_props(map1, map2);
    }

    // Returns a pair of permutations of a set of unique entries.
    fn test_entry_permutation_strategy(
        size: impl Into<SizeRange>,
    ) -> impl Strategy<Value = (Vec<TestEntry>, Vec<TestEntry>)> {
        prop::collection::vec(any::<TestEntry>(), size.into()).prop_perturb(
            |v, mut rng| {
                // It is possible (likely even) that the input vector has
                // duplicates. How can we remove them? The easiest way is to use
                // the TriHashMap logic that already exists to check for
                // duplicates. Insert all the entries one by one, then get the
                // list.
                let mut map = TriHashMap::<TestEntry>::new();
                for entry in v {
                    // The error case here is expected -- we're actively
                    // de-duping entries right now.
                    _ = map.insert_unique(entry);
                }
                let set = map.entries.into_vec();

                // Now shuffle the entries. This is a simple Fisher-Yates
                // shuffle (Durstenfeld variant, low to high).
                let mut set2 = set.clone();
                if set.len() < 2 {
                    return (set, set2);
                }
                for i in 0..set2.len() - 2 {
                    let j = rng.gen_range(i..set2.len());
                    set2.swap(i, j);
                }

                (set, set2)
            },
        )
    }

    // Test various conditions for non-equality.
    //
    // It's somewhat hard to capture mutations in a proptest (partly because
    // `TriMap` doesn't support mutating existing entries at the moment), so
    // this is a small example-based test.
    #[test]
    fn test_permutation_eq_examples() {
        let mut map1 = TriHashMap::<TestEntry>::new();
        let mut map2 = TriHashMap::<TestEntry>::new();

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

    /// Assert equality properties.
    ///
    /// The PartialEq algorithm is not obviously symmetric or reflexive, so we
    /// must ensure in our tests that it is.
    #[allow(clippy::eq_op)]
    fn assert_eq_props<T: Eq + fmt::Debug>(a: T, b: T) {
        assert_eq!(a, a, "a == a");
        assert_eq!(b, b, "b == b");
        assert_eq!(a, b, "a == b");
        assert_eq!(b, a, "b == a");
    }

    /// Assert inequality properties.
    ///
    /// The PartialEq algorithm is not obviously symmetric or reflexive, so we
    /// must ensure in our tests that it is.
    #[allow(clippy::eq_op)]
    fn assert_ne_props<T: Eq + fmt::Debug>(a: T, b: T) {
        // Also check reflexivity while we're here.
        assert_eq!(a, a, "a == a");
        assert_eq!(b, b, "b == b");
        assert_ne!(a, b, "a != b");
        assert_ne!(b, a, "b != a");
    }

}
