// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use iddqd::{
    internal::ValidateCompact, tri_hash_map::RefMut, TriHashItem, TriHashMap,
};
use iddqd_test_utils::{
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_item::{assert_iter_eq, test_item_permutation_strategy, TestItem},
};
use proptest::prelude::*;
use test_strategy::{proptest, Arbitrary};

#[test]
fn with_capacity() {
    let map = TriHashMap::<TestItem>::with_capacity(1024);
    assert_eq!(map.capacity(), 1024);
}

#[test]
fn test_insert_unique() {
    let mut map = TriHashMap::<TestItem>::new();

    // Add an element.
    let v1 = TestItem {
        key1: 0,
        key2: 'a',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    map.insert_unique(v1.clone()).unwrap();

    // Add an exact duplicate, which should error out.
    let error = map.insert_unique(v1.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v1);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key1, which should error out.
    let v2 = TestItem {
        key1: 0,
        key2: 'b',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key2, which should error out.
    let v3 = TestItem {
        key1: 1,
        key2: 'a',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v3.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v3);

    // Add a duplicate against just key3, which should error out.
    let v4 = TestItem {
        key1: 1,
        key2: 'b',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v4);

    // Add an item that doesn't have any conflicts.
    let v5 = TestItem {
        key1: 1,
        key2: 'b',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    map.insert_unique(v5.clone()).unwrap();

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    let mut items: Vec<RefMut<_>> = map.iter_mut().collect();
    items.sort_by_key(|e| e.key1());
    let e1 = &items[0];
    assert_eq!(**e1, v1);

    // Test that the RefMut Debug impl looks good.
    assert_eq!(
        format!("{:?}", e1),
        r#"TestItem { key1: 0, key2: 'a', key3: "x", value: "v" }"#,
    );

    let e2 = &*items[1];
    assert_eq!(*e2, v5);
}

// Example-based test for insert_overwrite.
//
// Can be used to write down examples seen from the property-based operation
// test, for easier debugging.
#[test]
fn test_insert_overwrite() {
    let mut map = TriHashMap::<TestItem>::new();

    // Add an element.
    let v1 = TestItem {
        key1: 20,
        key2: 'a',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    assert_eq!(map.insert_overwrite(v1.clone()), Vec::<TestItem>::new());

    // Add an element with the same keys but a different value.
    let v2 = TestItem {
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
    InsertUnique(TestItem),
    #[weight(2)]
    InsertOverwrite(TestItem),
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
            // The act of removing items, including calls to insert_overwrite,
            // can make the map non-compact.
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
    let mut map = TriHashMap::<TestItem>::new();
    let mut naive_map = NaiveMap::new_key123();

    let mut compactness = ValidateCompact::Compact;

    // Now perform the operations on both maps.
    for op in ops.into_iter() {
        if compactness == ValidateCompact::Compact && !op.remains_compact() {
            compactness = ValidateCompact::NonCompact;
        }

        match op {
            Operation::InsertUnique(item) => {
                let map_res = map.insert_unique(item.clone());
                let naive_res = naive_map.insert_unique(item.clone());

                assert_eq!(
                    map_res.is_ok(),
                    naive_res.is_ok(),
                    "map and naive map should agree on insert result"
                );
                if let Err(map_err) = map_res {
                    let naive_err = naive_res.unwrap_err();
                    assert_eq!(map_err.new_item(), naive_err.new_item());
                    assert_eq!(map_err.duplicates(), naive_err.duplicates(),);
                }

                map.validate(compactness).expect("map should be valid");
            }
            Operation::InsertOverwrite(item) => {
                let mut map_dups = map.insert_overwrite(item.clone());
                map_dups.sort();
                let mut naive_dups = naive_map.insert_overwrite(item.clone());
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
        let mut naive_items = naive_map.iter().collect::<Vec<_>>();
        naive_items.sort_by_key(|e| e.key1());

        assert_iter_eq(map.clone(), naive_items);
    }
}

#[proptest(cases = 64)]
fn proptest_permutation_eq(
    #[strategy(test_item_permutation_strategy::<TriHashMap<TestItem>>(0..256))]
    items: (Vec<TestItem>, Vec<TestItem>),
) {
    let (items1, items2) = items;
    let mut map1 = TriHashMap::<TestItem>::new();
    let mut map2 = TriHashMap::<TestItem>::new();

    for item in items1 {
        map1.insert_unique(item.clone()).unwrap();
    }
    for item in items2 {
        map2.insert_unique(item.clone()).unwrap();
    }

    assert_eq_props(map1, map2);
}

// Test various conditions for non-equality.
//
// It's a bit difficult to capture mutations in a proptest, so this is a small
// example-based test.
#[test]
fn test_permutation_eq_examples() {
    let mut map1 = TriHashMap::<TestItem>::new();
    let mut map2 = TriHashMap::<TestItem>::new();

    // Two empty maps are equal.
    assert_eq!(map1, map2);

    // Insert a single item into one map.
    let item = TestItem {
        key1: 0,
        key2: 'a',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    map1.insert_unique(item.clone()).unwrap();

    // The maps are not equal.
    assert_ne_props(&map1, &map2);

    // Insert the same item into the other map.
    map2.insert_unique(item.clone()).unwrap();

    // The maps are now equal.
    assert_eq_props(&map1, &map2);

    {
        // Insert an item with the same key2 and key3 but a different
        // key1.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem {
            key1: 2,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with the same key1 and key3 but a different
        // key2.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem {
            key1: 1,
            key2: 'c',
            key3: "y".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with the same key1 and key2 but a different
        // key3.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem {
            key1: 1,
            key2: 'b',
            key3: "z".to_string(),
            value: "v".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item where all the keys are the same, but the value is
        // different.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem {
            key1: 1,
            key2: 'b',
            key3: "y".to_string(),
            value: "w".to_string(),
        })
        .unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem {
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
    let mut map = TriHashMap::<TestItem>::new();
    map.insert_unique(TestItem {
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
    let mut map = TriHashMap::<TestItem>::new();
    map.insert_unique(TestItem {
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
    let mut map = TriHashMap::<TestItem>::new();
    map.insert_unique(TestItem {
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
        serde_utils::assert_serialize_roundtrip, test_item::TestItem,
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestItem>) {
        assert_serialize_roundtrip::<TriHashMap<TestItem>>(values);
    }
}
