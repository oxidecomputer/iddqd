// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::hash_table::MapHashTable;
use derive_where::derive_where;
use hashbrown::hash_table::{Entry, VacantEntry};
use serde::{Deserialize, Serialize, Serializer};
use std::{borrow::Borrow, collections::BTreeSet, fmt, hash::Hash};

/// An append-only 1:1:1 (trijective) map for three keys and a value.
///
/// The storage mechanism is a vector of entries, with indexes into that vector
/// stored in three hashmaps. This allows for efficient lookups by any of the
/// three keys, while preventing duplicates.
#[derive_where(Clone, Default)]
#[derive(Debug)]
pub struct TriHashMap<T: TriHashMapEntry> {
    entries: Vec<T>,
    // Invariant: the values (usize) in these maps are valid indexes into
    // `entries`, and are a 1:1 mapping.
    k1_to_entry: MapHashTable,
    k2_to_entry: MapHashTable,
    k3_to_entry: MapHashTable,
}

pub trait TriHashMapEntry: Clone {
    type K1<'a>: Eq + Hash
    where
        Self: 'a;
    type K2<'a>: Eq + Hash
    where
        Self: 'a;
    type K3<'a>: Eq + Hash
    where
        Self: 'a;

    fn key1(&self) -> Self::K1<'_>;
    fn key2(&self) -> Self::K2<'_>;
    fn key3(&self) -> Self::K3<'_>;
}

impl<T: TriHashMapEntry> TriHashMap<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            k1_to_entry: MapHashTable::new(),
            k2_to_entry: MapHashTable::new(),
            k3_to_entry: MapHashTable::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            k1_to_entry: MapHashTable::with_capacity(capacity),
            k2_to_entry: MapHashTable::with_capacity(capacity),
            k3_to_entry: MapHashTable::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter()
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[cfg(test)]
    fn validate(&self) -> anyhow::Result<()> {
        use anyhow::Context;

        // Check that all the maps are of the right size.
        self.k1_to_entry
            .validate(self.entries.len())
            .context("k1_to_entry failed validation")?;
        self.k2_to_entry
            .validate(self.entries.len())
            .context("k2_to_entry failed validation")?;
        self.k3_to_entry
            .validate(self.entries.len())
            .context("k3_to_entry failed validation")?;

        // Check that the indexes are all correct.
        for (ix, entry) in self.entries.iter().enumerate() {
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
                    "entry at index {} has mismatched indexes: key1: {}, key2: {}, key3: {}",
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
    pub fn insert_no_dups(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateEntry<T>> {
        let mut dups = BTreeSet::new();

        // Check for duplicates *before* inserting the new entry, because we
        // don't want to partially insert the new entry and then have to roll
        // back.
        let (e1, e2, e3) = {
            let k1 = value.key1();
            let k2 = value.key2();
            let k3 = value.key3();

            let e1 = detect_dup_or_insert(
                self.k1_to_entry.entry(k1, |index| self.entries[index].key1()),
                &mut dups,
            );
            eprint!("for k2: ");
            let e2 = detect_dup_or_insert(
                self.k2_to_entry.entry(k2, |index| self.entries[index].key2()),
                &mut dups,
            );
            let e3 = detect_dup_or_insert(
                self.k3_to_entry.entry(k3, |index| self.entries[index].key3()),
                &mut dups,
            );
            (e1, e2, e3)
        };

        if !dups.is_empty() {
            return Err(DuplicateEntry {
                new: value,
                dups: dups.iter().map(|ix| self.entries[*ix].clone()).collect(),
            });
        }

        let next_index = self.entries.len();
        // e1, e2 and e3 are all Some because if they were None, dups would be
        // non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);
        e3.unwrap().insert(next_index);
        self.entries.push(value);

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

    pub fn get2<'a, Q>(&'a self, key2: &Q) -> Option<&'a T>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find2(key2)
    }

    pub fn get3<'a, Q>(&'a self, key3: &Q) -> Option<&'a T>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Eq + Hash + ?Sized,
    {
        self.find3(key3)
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
        self.k1_to_entry
            .find_index(&self.entries, k, |entry| entry.key1().borrow() == k)
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
        self.k2_to_entry
            .find_index(&self.entries, k, |entry| entry.key2().borrow() == k)
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
        self.k3_to_entry
            .find_index(&self.entries, k, |entry| entry.key3().borrow() == k)
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
        for entry in &self.entries {
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

/// The `Serialize` impl for `TriHashMap` serializes just the list of entries.
impl<T: TriHashMapEntry> Serialize for TriHashMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Serialize just the entries -- don't serialize the indexes. We'll
        // rebuild the indexes on deserialization.
        self.entries.serialize(serializer)
    }
}

/// The `Deserialize` impl for `TriHashMap` deserializes the list of entries and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: TriHashMapEntry + fmt::Debug> Deserialize<'de> for TriHashMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // First, deserialize the entries.
        let entries = Vec::<T>::deserialize(deserializer)?;

        // Now build a map from scratch, inserting the entries sequentially.
        // This will catch issues with duplicates.
        let mut map = TriHashMap::new();
        for entry in entries {
            map.insert_no_dups(entry).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}

fn detect_dup_or_insert<'a>(
    entry: Entry<'a, usize>,
    dups: &mut BTreeSet<usize>,
) -> Option<VacantEntry<'a, usize>> {
    match entry {
        Entry::Vacant(slot) => Some(slot),
        Entry::Occupied(slot) => {
            dups.insert(*slot.get());
            None
        }
    }
}

#[derive(Debug)]
pub struct DuplicateEntry<T: TriHashMapEntry> {
    new: T,
    dups: Vec<T>,
}

impl<T: TriHashMapEntry + fmt::Debug> fmt::Display for DuplicateEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "duplicate entry: {:?} conflicts with existing: {:?}",
            self.new, self.dups
        )
    }
}

impl<T: TriHashMapEntry + fmt::Debug> std::error::Error for DuplicateEntry<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use prop::sample::SizeRange;
    use proptest::prelude::*;
    use test_strategy::{proptest, Arbitrary};

    #[derive(
        Clone, Debug, Eq, PartialEq, Arbitrary, Serialize, Deserialize,
    )]
    struct TestEntry {
        key1: u8,
        key2: char,
        key3: String,
        value: String,
    }

    impl TriHashMapEntry for TestEntry {
        // These types are chosen to represent various kinds of keys in the
        // proptest below.
        //
        // We use u8 since there can only be 256 values, increasing the
        // likelihood of collisions in the proptest below.
        type K1<'a> = u8;
        // char is chosen because the Arbitrary impl for it is biased towards
        // ASCII, increasing the likelihood of collisions.
        type K2<'a> = char;
        // &str is a generally open-ended type that probably won't have many
        // collisions.
        type K3<'a> = &'a str;

        fn key1(&self) -> Self::K1<'_> {
            self.key1
        }

        fn key2(&self) -> Self::K2<'_> {
            self.key2
        }

        fn key3(&self) -> Self::K3<'_> {
            self.key3.as_str()
        }
    }

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
        map.insert_no_dups(v1.clone()).unwrap();

        // Add an exact duplicate, which should error out.
        let error = map.insert_no_dups(v1.clone()).unwrap_err();
        assert_eq!(&error.new, &v1);
        assert_eq!(error.dups, vec![v1.clone()]);

        // Add a duplicate against just key1, which should error out.
        let v2 = TestEntry {
            key1: 0,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_no_dups(v2.clone()).unwrap_err();
        assert_eq!(&error.new, &v2);
        assert_eq!(error.dups, vec![v1.clone()]);

        // Add a duplicate against just key2, which should error out.
        let v3 = TestEntry {
            key1: 1,
            key2: 'a',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_no_dups(v3.clone()).unwrap_err();
        assert_eq!(&error.new, &v3);

        // Add a duplicate against just key3, which should error out.
        let v4 = TestEntry {
            key1: 1,
            key2: 'b',
            key3: "x".to_string(),
            value: "v".to_string(),
        };
        let error = map.insert_no_dups(v4.clone()).unwrap_err();
        assert_eq!(&error.new, &v4);

        // Add an entry that doesn't have any conflicts.
        let v5 = TestEntry {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        };
        map.insert_no_dups(v5.clone()).unwrap();
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
        ) -> Result<(), DuplicateEntry<TestEntry>> {
            let dups = self
                .entries
                .iter()
                .filter(|e| {
                    e.key1 == entry.key1
                        || e.key2 == entry.key2
                        || e.key3 == entry.key3
                })
                .cloned()
                .collect::<Vec<_>>();

            if !dups.is_empty() {
                return Err(DuplicateEntry { new: entry, dups });
            }

            self.entries.push(entry);
            Ok(())
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

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestEntry>) {
        let mut map = TriHashMap::<TestEntry>::new();
        let mut first_error = None;
        for value in values.clone() {
            // Ignore errors from duplicates which are quite possible to occur
            // here, since we're just testing serialization. But store the
            // first error to ensure that deserialization returns errors.
            if let Err(error) = map.insert_no_dups(value) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        let serialized = serde_json::to_string(&map).unwrap();
        let deserialized: TriHashMap<TestEntry> =
            serde_json::from_str(&serialized).unwrap();

        assert_eq!(map.entries, deserialized.entries, "entries match");
        deserialized.validate().expect("deserialized map is valid");

        // Try deserializing the full list of values directly, and see that the
        // error reported is the same as first_error.
        //
        // Here we rely on the fact that a TriMap is serialized as just a
        // vector.
        let serialized = serde_json::to_string(&values).unwrap();
        let res: Result<TriHashMap<TestEntry>, _> =
            serde_json::from_str(&serialized);
        match (first_error, res) {
            (None, Ok(_)) => {} // No error, should be fine
            (Some(first_error), Ok(_)) => {
                panic!("expected error ({first_error}), but deserialization succeeded")
            }
            (None, Err(error)) => {
                panic!("unexpected error: {error}, deserialization should have succeeded")
            }
            (Some(first_error), Err(error)) => {
                // first_error is the error from the map, and error is the
                // deserialization error (which should always be a custom
                // error, stored as a string).
                let expected = first_error.to_string();
                let actual = error.to_string();
                assert_eq!(actual, expected, "error matches");
            }
        }
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
                    let map_res = map.insert_no_dups(entry.clone());
                    let naive_res =
                        naive_map.insert_entry_no_dups(entry.clone());

                    assert_eq!(map_res.is_ok(), naive_res.is_ok());
                    if let Err(map_err) = map_res {
                        let naive_err = naive_res.unwrap_err();
                        assert_eq!(map_err.new, naive_err.new);
                        assert_eq!(map_err.dups, naive_err.dups);
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
            map1.insert_no_dups(entry.clone()).unwrap();
        }
        for entry in entries2 {
            map2.insert_no_dups(entry.clone()).unwrap();
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
                // duplicates. How can we remove them? The easiest way is to
                // use the TriMap logic that already exists to check for
                // duplicates. Insert all the entries one by one, then get the
                // list.
                let mut map = TriHashMap::<TestEntry>::new();
                for entry in v {
                    // The error case here is expected -- we're actively
                    // de-duping entries right now.
                    _ = map.insert_no_dups(entry);
                }
                let v = map.entries;

                // Now shuffle the entries. This is a simple Fisher-Yates
                // shuffle (Durstenfeld variant, low to high).
                let mut v2 = v.clone();
                if v.len() < 2 {
                    return (v, v2);
                }
                for i in 0..v2.len() - 2 {
                    let j = rng.gen_range(i..v2.len());
                    v2.swap(i, j);
                }

                (v, v2)
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
        map1.insert_no_dups(entry.clone()).unwrap();

        // The maps are not equal.
        assert_ne_props(&map1, &map2);

        // Insert the same entry into the other map.
        map2.insert_no_dups(entry.clone()).unwrap();

        // The maps are now equal.
        assert_eq_props(&map1, &map2);

        {
            // Insert an entry with the same key2 and key3 but a different
            // key1.
            let mut map1 = map1.clone();
            map1.insert_no_dups(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_no_dups(TestEntry {
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
            map1.insert_no_dups(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_no_dups(TestEntry {
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
            map1.insert_no_dups(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "v".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_no_dups(TestEntry {
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
            map1.insert_no_dups(TestEntry {
                key1: 1,
                key2: 'b',
                key3: "y".to_string(),
                value: "w".to_string(),
            })
            .unwrap();
            assert_ne_props(&map1, &map2);

            let mut map2 = map2.clone();
            map2.insert_no_dups(TestEntry {
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
