// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use iddqd::{
    id_ord_map::{Entry, RefMut},
    internal::ValidateCompact,
    IdOrdItem, IdOrdMap,
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
    let map = IdOrdMap::<TestItem>::with_capacity(1024);
    assert!(map.capacity() >= 1024);
}

#[test]
fn test_insert_unique() {
    let mut map = IdOrdMap::<TestItem>::new();

    // Add an element.
    let v1 = TestItem {
        key1: 20,
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
        key1: 20,
        key2: 'b',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against key2. IdOrdMap only uses key1 here, so this
    // should be allowed.
    let v3 = TestItem {
        key1: 5,
        key2: 'a',
        key3: "y".to_string(),
        value: "v".to_string(),
    };
    map.insert_unique(v3.clone()).unwrap();

    // Add a duplicate against key1, which should error out.
    let v4 = TestItem {
        key1: 5,
        key2: 'b',
        key3: "x".to_string(),
        value: "v".to_string(),
    };
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v4);

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    let items: Vec<RefMut<_>> = map.iter_mut().collect();
    let e1 = &items[0];
    assert_eq!(**e1, v3);

    // Test that the RefMut Debug impl looks good.
    assert_eq!(
        format!("{:?}", e1),
        r#"TestItem { key1: 5, key2: 'a', key3: "y", value: "v" }"#,
    );

    let e2 = &*items[1];
    assert_eq!(*e2, v1);
}

#[derive(Debug, Arbitrary)]
enum Operation {
    // Make inserts a bit more common to try and fill up the map.
    #[weight(3)]
    InsertUnique(TestItem),
    #[weight(2)]
    InsertOverwrite(TestItem),
    Get(u8),
    Remove(u8),
}

impl Operation {
    fn remains_compact(&self) -> bool {
        match self {
            Operation::InsertUnique(_) | Operation::Get(_) => true,
            // The act of removing items, including calls to insert_overwrite,
            // can make the map non-compact.
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
    let mut map = IdOrdMap::<TestItem>::new();
    let mut naive_map = NaiveMap::new_key1();

    let mut compactness = ValidateCompact::Compact;

    // Now perform the operations on both maps.
    for op in ops {
        if compactness == ValidateCompact::Compact && !op.remains_compact() {
            compactness = ValidateCompact::NonCompact;
        }

        match op {
            Operation::InsertUnique(item) => {
                let map_res = map.insert_unique(item.clone());
                let naive_res = naive_map.insert_unique(item.clone());

                assert_eq!(map_res.is_ok(), naive_res.is_ok());
                if let Err(map_err) = map_res {
                    let naive_err = naive_res.unwrap_err();
                    assert_eq!(map_err.new_item(), naive_err.new_item());
                    assert_eq!(map_err.duplicates(), naive_err.duplicates());
                }

                map.validate(compactness).expect("map should be valid");
            }
            Operation::InsertOverwrite(item) => {
                let map_dups = map.insert_overwrite(item.clone());
                let mut naive_dups = naive_map.insert_overwrite(item.clone());
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
        let mut naive_items = naive_map.iter().collect::<Vec<_>>();
        naive_items.sort_by_key(|e| *e.key());

        assert_iter_eq(map.clone(), naive_items);
    }
}

#[proptest(cases = 64)]
fn proptest_permutation_eq(
    #[strategy(test_item_permutation_strategy::<IdOrdMap<TestItem>>(0..PERMUTATION_LEN))]
    items: (Vec<TestItem>, Vec<TestItem>),
) {
    let (items1, items2) = items;
    let mut map1 = IdOrdMap::<TestItem>::new();
    let mut map2 = IdOrdMap::<TestItem>::new();

    for item in items1.clone() {
        map1.insert_unique(item.clone()).unwrap();
    }
    for item in items2.clone() {
        map2.insert_unique(item.clone()).unwrap();
    }

    assert_eq_props(&map1, &map2);

    // Also test from_iter_unique.
    let map3 = IdOrdMap::from_iter_unique(items1).unwrap();
    let map4 = IdOrdMap::from_iter_unique(items2).unwrap();
    assert_eq_props(&map1, &map3);
    assert_eq_props(&map3, &map4);
}

// Test various conditions for non-equality.
#[test]
fn test_permutation_eq_examples() {
    let mut map1 = IdOrdMap::<TestItem>::new();
    let mut map2 = IdOrdMap::<TestItem>::new();

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
        // Insert an item with the same key1 and key3 but a different key2.
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
        // Insert an item with the same key1 and key2 but a different key3.
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
#[should_panic(expected = "key changed during RefMut borrow")]
fn get_mut_panics_if_key_changes() {
    let mut map = IdOrdMap::<TestItem>::new();
    map.insert_unique(TestItem {
        key1: 128,
        key2: 'b',
        key3: "y".to_owned(),
        value: "x".to_owned(),
    })
    .unwrap();
    map.get_mut(&128).unwrap().key1 = 2;
}

#[test]
fn entry_examples() {
    let mut map = IdOrdMap::<TestItem>::new();
    let item1 = TestItem {
        key1: 0,
        key2: 'a',
        key3: "x".to_string(),
        value: "v".to_string(),
    };

    let Entry::Vacant(entry) = map.entry(item1.key()) else {
        panic!("expected VacantEntry")
    };
    let mut entry = entry.insert_entry(item1.clone());

    assert_eq!(entry.get(), &item1);
    assert_eq!(entry.get_mut().into_ref(), &item1);
    assert_eq!(entry.into_ref(), &item1);

    // Try looking up another item with the same key1.
    let item2 = TestItem {
        key1: 0, // Same key1 as item1
        key2: 'b',
        key3: "y".to_string(),
        value: "x".to_string(),
    };

    let Entry::Occupied(mut entry) = map.entry(item2.key()) else {
        panic!("expected OccupiedEntry");
    };
    assert_eq!(entry.insert(item2.clone()), item1);

    assert_eq!(entry.remove(), item2);

    // Put item2 back in via the Entry API.
    let item2_mut = map.entry(item2.key()).or_insert(item2.clone());
    assert_eq!(item2_mut.into_ref(), &item2);

    // Add another item using or_insert_with.
    let item3 = TestItem {
        key1: 1,
        key2: 'b',
        key3: "y".to_string(),
        value: "x".to_string(),
    };
    let item3_mut = map.entry(item3.key()).or_insert_with(|| item3.clone());
    assert_eq!(item3_mut.into_ref(), &item3);

    // item4 is similar to item3 except with a different value.
    let item4 = TestItem {
        key1: 1, // Same key1 as item3
        key2: 'b',
        key3: "y".to_string(),
        value: "some-other-value".to_string(),
    };
    // item4 should *not* be inserted via this path.
    let item3_mut = map.entry(item4.key()).or_insert(item4.clone());
    assert_eq!(item3_mut.into_ref(), &item3);

    // Similarly, item4 should *not* be inserted via the or_insert_with path.
    let item3_mut = map
        .entry(item4.key())
        .or_insert_with(|| panic!("or_insert_with called for existing key"));
    assert_eq!(item3_mut.into_ref(), &item3);

    // Add another item using or_insert_ref.
    let item5 = TestItem {
        key1: 2,
        key2: 'c',
        key3: "z".to_string(),
        value: "w".to_string(),
    };
    let item5_ref = map.entry(item5.key()).or_insert_ref(item5.clone());
    assert_eq!(item5_ref, &item5);

    // Add another item using or_insert_with_ref.
    let item6 = TestItem {
        key1: 3,
        key2: 'd',
        key3: "a".to_string(),
        value: "b".to_string(),
    };
    let item6_ref = map.entry(item6.key()).or_insert_with_ref(|| item6.clone());
    assert_eq!(item6_ref, &item6);

    // item7 is similar to item5 except with a different value.
    let item7 = TestItem {
        key1: 2, // Same key1 as item5
        key2: 'c',
        key3: "z".to_string(),
        value: "yet-another-value".to_string(),
    };
    // item7 should *not* be inserted via this path.
    let item5_ref = map.entry(item7.key()).or_insert_ref(item7.clone());
    assert_eq!(item5_ref, &item5);

    // Similarly, item7 should *not* be inserted via the or_insert_with_ref
    // path.
    let entry = map.entry(item7.key()).or_insert_with_ref(|| {
        panic!("or_insert_with_ref called for existing key")
    });
    assert_eq!(entry, &item5);

    // The and_modify path should be called, however.
    let mut and_modify_called = false;
    map.entry(item4.key()).and_modify(|_| and_modify_called = true);
    assert!(and_modify_called);
}

#[test]
#[should_panic = "key already present in map"]
fn or_insert_ref_panics_for_present_key() {
    let v1 = TestItem {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdOrdMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem {
        key1: 1,
        key2: 'a',
        key3: "bar".to_owned(),
        value: "value".to_owned(),
    };
    let entry = map.entry(v2.key());
    assert!(matches!(entry, Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert_ref(v1);
}

#[test]
#[should_panic = "key already present in map"]
fn or_insert_panics_for_present_key() {
    let v1 = TestItem {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdOrdMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem {
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
fn insert_entry_panics_for_present_key() {
    let v1 = TestItem {
        key1: 0,
        key2: 'a',
        key3: "foo".to_owned(),
        value: "value".to_owned(),
    };
    let mut map = IdOrdMap::new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem {
        key1: 1,
        key2: 'a',
        key3: "bar".to_owned(),
        value: "value".to_owned(),
    };
    let entry = map.entry(v2.key());
    assert!(matches!(entry, Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    if let Entry::Vacant(vacant_entry) = entry {
        vacant_entry.insert_entry(v1);
    } else {
        panic!("Expected Vacant entry");
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use iddqd::IdOrdMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip, test_item::TestItem,
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestItem>) {
        assert_serialize_roundtrip::<IdOrdMap<TestItem>>(values);
    }
}
