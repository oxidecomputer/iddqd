// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use iddqd::{internal::ValidateCompact, TriHashItem, TriHashMap};
use iddqd_test_utils::{
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_entry::{assert_iter_eq, test_entry_permutation_strategy, TestEntry},
};
use proptest::prelude::*;
use test_strategy::{proptest, Arbitrary};

#[test]
fn test_insert_unique() {
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
    assert_eq!(error.new_entry(), &v1);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key1, which should error out.
    let v2 = TestEntry {
        key1: 0,
        key2: 'b',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_entry(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key2, which should error out.
    let v3 = TestEntry {
        key1: 1,
        key2: 'a',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v3.clone()).unwrap_err();
    assert_eq!(error.new_entry(), &v3);

    // Add a duplicate against just key3, which should error out.
    let v4 = TestEntry {
        key1: 1,
        key2: 'b',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_entry(), &v4);

    // Add an entry that doesn't have any conflicts.
    let v5 = TestEntry {
        key1: 1,
        key2: 'b',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    map.insert_unique(v5.clone()).unwrap();
}

// Example-based test for insert_overwrite.
//
// Can be used to write down examples seen from the property-based operation
// test, for easier debugging.
#[test]
fn test_insert_overwrite() {
    let mut map = TriHashMap::<TestEntry>::new();

    // Add an element.
    let v1 = TestEntry {
        key1: 20,
        key2: 'a',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    assert_eq!(map.insert_overwrite(v1.clone()), Vec::<TestEntry>::new());

    // Add an element with the same keys but a different value.
    let v2 = TestEntry {
        key1: 20,
        key2: 'a',
        key3: "x".to_string(),
        value: "w".to_string(),
    };
    assert_eq!(map.insert_overwrite(v2.clone()), vec![v1]);

    map.validate(ValidateCompact::NonCompact).expect("validation failed");
}

#[derive(Debug, Arbitrary)]
enum Operation {
    // Make inserts a bit more common to try and fill up the map.
    #[weight(3)]
    InsertUnique(TestEntry),
    #[weight(2)]
    InsertOverwrite(TestEntry),
    Get1(u8),
    Get2(char),
    Get3(String),
    Remove1(u8),
    Remove2(char),
    Remove3(String),
}

impl Operation {
    fn remains_compact(&self) -> bool {
        match self {
            Operation::InsertUnique(_)
            | Operation::Get1(_)
            | Operation::Get2(_)
            | Operation::Get3(_) => true,
            // The act of removing entries, including calls to
            // insert_overwrite, can make the map non-compact.
            Operation::InsertOverwrite(_)
            | Operation::Remove1(_)
            | Operation::Remove2(_)
            | Operation::Remove3(_) => false,
        }
    }
}

#[proptest(cases = 16)]
fn proptest_ops(
    #[strategy(prop::collection::vec(any::<Operation>(), 0..1024))] ops: Vec<
        Operation,
    >,
) {
    let mut map = TriHashMap::<TestEntry>::new();
    let mut naive_map = NaiveMap::new_key123();

    let mut compactness = ValidateCompact::Compact;

    // Now perform the operations on both maps.
    for op in ops.into_iter() {
        if compactness == ValidateCompact::Compact && !op.remains_compact() {
            compactness = ValidateCompact::NonCompact;
        }

        match op {
            Operation::InsertUnique(entry) => {
                let map_res = map.insert_unique(entry.clone());
                let naive_res = naive_map.insert_unique(entry.clone());

                assert_eq!(
                    map_res.is_ok(),
                    naive_res.is_ok(),
                    "map and naive map should agree on insert result"
                );
                if let Err(map_err) = map_res {
                    let naive_err = naive_res.unwrap_err();
                    assert_eq!(map_err.new_entry(), naive_err.new_entry());
                    assert_eq!(map_err.duplicates(), naive_err.duplicates(),);
                }

                map.validate(compactness).expect("map should be valid");
            }
            Operation::InsertOverwrite(entry) => {
                let mut map_dups = map.insert_overwrite(entry.clone());
                map_dups.sort();
                let mut naive_dups = naive_map.insert_overwrite(entry.clone());
                naive_dups.sort();

                assert_eq!(
                    map_dups, naive_dups,
                    "map and naive map should agree on insert_overwrite dups"
                );
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Get1(key1) => {
                let map_res = map.get1(&key1);
                let naive_res = naive_map.get1(key1);

                assert_eq!(map_res, naive_res);
            }
            Operation::Get2(key2) => {
                let map_res = map.get2(&key2);
                let naive_res = naive_map.get2(key2);

                assert_eq!(map_res, naive_res);
            }
            Operation::Get3(key3) => {
                let map_res = map.get3(key3.as_str());
                let naive_res = naive_map.get3(&key3);

                assert_eq!(map_res, naive_res);
            }
            Operation::Remove1(key1) => {
                let map_res = map.remove1(key1);
                let naive_res = naive_map.remove1(key1);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Remove2(key2) => {
                let map_res = map.remove2(key2);
                let naive_res = naive_map.remove2(key2);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Remove3(key3) => {
                let map_res = map.remove3(key3.as_str());
                let naive_res = naive_map.remove3(&key3);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
        }

        // Check that the iterators work correctly.
        let mut naive_entries = naive_map.iter().collect::<Vec<_>>();
        naive_entries.sort_by_key(|e| e.key1());

        assert_iter_eq(map.clone(), naive_entries);
    }
}

#[proptest(cases = 64)]
fn proptest_permutation_eq(
    #[strategy(test_entry_permutation_strategy::<TriHashMap<TestEntry>>(0..256))]
    entries: (Vec<TestEntry>, Vec<TestEntry>),
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

#[test]
#[should_panic(expected = "key1 changed during RefMut borrow")]
fn get_mut_panics_if_key1_changes() {
    let mut map = TriHashMap::<TestEntry>::new();
    map.insert_unique(TestEntry {
        key1: 128,
        key2: 'b',
        key3: "y".to_owned(),
        value: "x".to_owned(),
    })
    .unwrap();
    map.get1_mut(128).unwrap().key1 = 2;
}

#[test]
#[should_panic(expected = "key2 changed during RefMut borrow")]
fn get_mut_panics_if_key2_changes() {
    let mut map = TriHashMap::<TestEntry>::new();
    map.insert_unique(TestEntry {
        key1: 128,
        key2: 'b',
        key3: "y".to_owned(),
        value: "x".to_owned(),
    })
    .unwrap();
    map.get1_mut(128).unwrap().key2 = 'c';
}

#[test]
#[should_panic(expected = "key3 changed during RefMut borrow")]
fn get_mut_panics_if_key3_changes() {
    let mut map = TriHashMap::<TestEntry>::new();
    map.insert_unique(TestEntry {
        key1: 128,
        key2: 'b',
        key3: "y".to_owned(),
        value: "x".to_owned(),
    })
    .unwrap();
    map.get1_mut(128).unwrap().key3 = "z".to_owned();
}

#[cfg(feature = "serde")]
mod serde_tests {
    use iddqd::TriHashMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip, test_entry::TestEntry,
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestEntry>) {
        assert_serialize_roundtrip::<TriHashMap<TestEntry>>(values);
    }
}
