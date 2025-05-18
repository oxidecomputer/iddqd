// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use iddqd::{
    id_btree_map::{Entry, RefMut},
    internal::ValidateCompact,
    IdBTreeMap, IdBTreeMapEntry,
};
use iddqd_test_utils::{
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_entry::{assert_iter_eq, test_entry_permutation_strategy, TestEntry},
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

#[derive(Debug, Arbitrary)]
enum Operation {
    // Make inserts a bit more common to try and fill up the map.
    #[weight(3)]
    InsertUnique(TestEntry),
    #[weight(2)]
    InsertOverwrite(TestEntry),
    Get(u8),
    Remove(u8),
}

impl Operation {
    fn remains_compact(&self) -> bool {
        match self {
            Operation::InsertUnique(_) | Operation::Get(_) => true,
            // The act of removing entries, including calls to
            // insert_overwrite, can make the map non-compact.
            Operation::InsertOverwrite(_) | Operation::Remove(_) => false,
        }
    }
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
    #[strategy(prop::collection::vec(any::<Operation>(), 0..OP_LEN))] ops: Vec<
        Operation,
    >,
) {
    let mut map = IdBTreeMap::<TestEntry>::new();
    let mut naive_map = NaiveMap::new_key1();

    let mut compactness = ValidateCompact::Compact;

    // Now perform the operations on both maps.
    for op in ops {
        if compactness == ValidateCompact::Compact && !op.remains_compact() {
            compactness = ValidateCompact::NonCompact;
        }

        match op {
            Operation::InsertUnique(entry) => {
                let map_res = map.insert_unique(entry.clone());
                let naive_res = naive_map.insert_unique(entry.clone());

                assert_eq!(map_res.is_ok(), naive_res.is_ok());
                if let Err(map_err) = map_res {
                    let naive_err = naive_res.unwrap_err();
                    assert_eq!(map_err.new_entry(), naive_err.new_entry());
                    assert_eq!(map_err.duplicates(), naive_err.duplicates());
                }

                map.validate(compactness).expect("map should be valid");
            }
            Operation::InsertOverwrite(entry) => {
                let map_dups = map.insert_overwrite(entry.clone());
                let mut naive_dups = naive_map.insert_overwrite(entry.clone());
                assert!(naive_dups.len() <= 1, "max one conflict");
                let naive_dup = naive_dups.pop();

                assert_eq!(
                    map_dups, naive_dup,
                    "map and naive map should agree on insert_overwrite dup"
                );
                map.validate(compactness).expect("map should be valid");
            }

            Operation::Get(key) => {
                let map_res = map.get(&key);
                let naive_res = naive_map.get1(key);

                assert_eq!(map_res, naive_res);
            }
            Operation::Remove(key) => {
                let map_res = map.remove(&key);
                let naive_res = naive_map.remove1(key);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
        }

        // Check that the iterators work correctly.
        let mut naive_entries = naive_map.iter().collect::<Vec<_>>();
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

#[test]
#[should_panic(expected = "key changed during RefMut borrow")]
fn get_mut_panics_if_key_changes() {
    let mut map = IdBTreeMap::<TestEntry>::new();
    map.insert_unique(TestEntry {
        key1: 128,
        key2: 'b',
        key3: "y".to_owned(),
        value: "x".to_owned(),
    })
    .unwrap();
    map.get_mut(&128).unwrap().key1 = 2;
}

#[test]
#[should_panic = "key already present in map"]
fn insert_panics_for_present_key() {
    let v1 = TestEntry {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdBTreeMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestEntry {
        key1: 1,
        key2: 'a',
        key3: "bar".to_owned(),
        value: "value".to_owned(),
    };
    let entry = map.entry(v2.key());
    assert!(matches!(entry, Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert(v1);
}

#[test]
#[should_panic = "key already present in map"]
fn insert_mut_panics_for_present_key() {
    let v1 = TestEntry {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdBTreeMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestEntry {
        key1: 1,
        key2: 'a',
        key3: "bar".to_owned(),
        value: "value".to_owned(),
    };
    let entry = map.entry(v2.key());
    assert!(matches!(entry, Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert_mut(v1);
}

#[test]
#[should_panic = "key already present in map"]
fn insert_entry_panics_for_present_key() {
    let v1 = TestEntry {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdBTreeMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestEntry {
        key1: 1,
        key2: 'a',
        key3: "bar".to_owned(),
        value: "value".to_owned(),
    };
    let entry = map.entry(v2.key());
    assert!(matches!(entry, Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.insert_entry(v1);
}

#[cfg(feature = "serde")]
mod serde_tests {
    use iddqd::IdBTreeMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip, test_entry::TestEntry,
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestEntry>) {
        assert_serialize_roundtrip::<IdBTreeMap<TestEntry>>(values);
    }
}
