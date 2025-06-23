use iddqd::{
    IdHashItem, IdHashMap, id_hash_map, id_upcast, internal::ValidateCompact,
};
use iddqd_test_utils::{
    borrowed_item::BorrowedItem,
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_item::{
        Alloc, HashBuilder, ItemMap, TestItem, TestKey1, assert_iter_eq,
        test_item_permutation_strategy,
    },
};
use proptest::prelude::*;
use std::path::Path;
use test_strategy::{Arbitrary, proptest};

#[derive(Clone, Debug)]
struct SimpleItem {
    key: u32,
}

impl IdHashItem for SimpleItem {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.key
    }

    id_upcast!();
}

#[test]
fn debug_impls() {
    let mut map = IdHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(SimpleItem { key: 1 }).unwrap();
    map.insert_unique(SimpleItem { key: 20 }).unwrap();
    map.insert_unique(SimpleItem { key: 10 }).unwrap();

    assert_eq!(
        format!("{map:?}"),
        // This is a small-enough map that the order of iteration is
        // deterministic.
        r#"{1: SimpleItem { key: 1 }, 10: SimpleItem { key: 10 }, 20: SimpleItem { key: 20 }}"#
    );
    assert_eq!(
        format!("{:?}", map.get_mut(&1).unwrap()),
        "SimpleItem { key: 1 }"
    );
}

#[test]
fn with_capacity() {
    let map = IdHashMap::<TestItem, HashBuilder>::with_capacity_and_hasher(
        1024,
        HashBuilder::default(),
    );
    assert!(map.capacity() >= 1024);
}

#[test]
fn test_insert_unique() {
    let mut map = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add an element.
    let v1 = TestItem::new(20, 'a', "x", "v");
    map.insert_unique(v1.clone()).unwrap();

    // Add an exact duplicate, which should error out.
    let error = map.insert_unique(v1.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v1);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key1, which should error out.
    let v2 = TestItem::new(20, 'b', "y", "v");
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against key2. IdHashMap only uses key1 here, so this
    // should be allowed.
    let v3 = TestItem::new(5, 'a', "y", "v");
    map.insert_unique(v3.clone()).unwrap();

    // Add a duplicate against key1, which should error out.
    let v4 = TestItem::new(5, 'b', "x", "v");
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v4);

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    let mut items: Vec<id_hash_map::RefMut<_, HashBuilder>> =
        map.iter_mut().collect();
    items.sort_by(|a, b| a.key().cmp(&b.key()));
    let e1 = &items[0];
    assert_eq!(**e1, v3);

    // Test that the RefMut Debug impl looks good.
    assert!(
        format!("{:?}", e1).starts_with(
            r#"TestItem { key1: 5, key2: 'a', key3: "y", value: "v""#,
        ),
        "RefMut Debug impl should forward to TestItem",
    );

    let e2 = &*items[1];
    assert_eq!(*e2, v1);
}

#[test]
fn test_extend() {
    let mut map = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let items = vec![
        TestItem::new(1, 'a', "x", "v"),
        TestItem::new(2, 'b', "y", "w"),
        TestItem::new(1, 'c', "z", "overwritten"), // duplicate key, should overwrite
    ];
    map.extend(items.clone());
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&TestKey1::new(&1)).unwrap().value, "overwritten");
    assert_eq!(map.get(&TestKey1::new(&2)).unwrap().value, "w");
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

#[proptest(cases = 16)]
fn proptest_ops(
    #[strategy(prop::collection::vec(any::<Operation>(), 0..1024))] ops: Vec<
        Operation,
    >,
) {
    let mut map = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
                let map_res = map.get(&TestKey1::new(&key));
                let naive_res = naive_map.get1(key);

                assert_eq!(map_res, naive_res);
            }
            Operation::Remove(key) => {
                let map_res = map.remove(&TestKey1::new(&key));
                let naive_res = naive_map.remove1(key);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
        }

        // Check that the iterators work correctly.
        let mut naive_items = naive_map.iter().collect::<Vec<_>>();
        naive_items.sort_by(|a, b| a.key().cmp(&b.key()));

        assert_iter_eq(map.clone(), naive_items);
    }
}

#[proptest(cases = 64)]
fn proptest_permutation_eq(
    #[strategy(test_item_permutation_strategy::<IdHashMap<TestItem, HashBuilder, Alloc>>(0..256))]
    items: (Vec<TestItem>, Vec<TestItem>),
) {
    let (items1, items2) = items;
    let mut map1 = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    for item in items1.clone() {
        map1.insert_unique(item.clone()).unwrap();
    }
    for item in items2.clone() {
        map2.insert_unique(item.clone()).unwrap();
    }

    assert_eq_props(&map1, &map2);
}

// Test various conditions for non-equality.
#[test]
fn test_permutation_eq_examples() {
    let mut map1 = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Two empty maps are equal.
    assert_eq!(map1, map2);

    // Insert a single item into one map.
    let item = TestItem::new(0, 'a', "x", "v");
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
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(2, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with the same key1 and key3 but a different key2.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(1, 'c', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with the same key1 and key2 but a different key3.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(1, 'b', "z", "v")).unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item where all the keys are the same, but the value is
        // different.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "w")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(1, 'b', "y", "x")).unwrap();
        assert_ne_props(&map1, &map2);
    }
}

#[test]
#[should_panic(expected = "key changed during RefMut borrow")]
fn get_mut_panics_if_key_changes() {
    let mut map = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get_mut(&TestKey1::new(&128)).unwrap().key1 = 2;
}

#[test]
fn entry_examples() {
    let mut map = IdHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let item1 = TestItem::new(0, 'a', "x", "v");

    let id_hash_map::Entry::Vacant(entry) = map.entry(item1.key()) else {
        panic!("expected VacantEntry")
    };
    let mut entry = entry.insert_entry(item1.clone());

    assert_eq!(entry.get(), &item1);
    assert_eq!(entry.get_mut().into_ref(), &item1);
    assert_eq!(entry.into_ref(), &item1);

    // Try looking up another item with the same key1.
    let item2 = TestItem::new(0, 'b', "y", "x");

    let id_hash_map::Entry::Occupied(mut entry) = map.entry(item2.key()) else {
        panic!("expected OccupiedEntry");
    };
    assert_eq!(entry.insert(item2.clone()), item1);

    assert_eq!(entry.remove(), item2);

    // Put item2 back in via the Entry API.
    let item2_mut = map.entry(item2.key()).or_insert(item2.clone());
    assert_eq!(item2_mut.into_ref(), &item2);

    // Add another item using or_insert_with.
    let item3 = TestItem::new(1, 'b', "y", "x");
    let item3_mut = map.entry(item3.key()).or_insert_with(|| item3.clone());
    assert_eq!(item3_mut.into_ref(), &item3);

    // item4 is similar to item3 except with a different value.
    let item4 = TestItem::new(1, 'b', "y", "some-other-value");
    // item4 should *not* be inserted via this path.
    let item3_mut = map.entry(item4.key()).or_insert(item4.clone());
    assert_eq!(item3_mut.into_ref(), &item3);

    // Similarly, item4 should *not* be inserted via the or_insert_with path.
    let item3_mut = map
        .entry(item4.key())
        .or_insert_with(|| panic!("or_insert_with called for existing key"));
    assert_eq!(item3_mut.into_ref(), &item3);

    // The and_modify path should be called, however.
    let mut and_modify_called = false;
    map.entry(item4.key()).and_modify(|_| and_modify_called = true);
    assert!(and_modify_called);
}

#[test]
#[should_panic = "key hashes do not match"]
fn insert_panics_for_non_matching_key() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = IdHashMap::<_, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key());
    assert!(matches!(entry, id_hash_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert(v1);
}

#[test]
#[should_panic = "key hashes do not match"]
fn insert_entry_panics_for_non_matching_key() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = IdHashMap::<_, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key());
    assert!(matches!(entry, id_hash_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    if let id_hash_map::Entry::Vacant(vacant_entry) = entry {
        vacant_entry.insert_entry(v1);
    } else {
        panic!("expected VacantEntry");
    }
}

#[test]
fn borrowed_item() {
    let mut map = IdHashMap::<BorrowedItem, HashBuilder, Alloc>::default();
    let item1 =
        BorrowedItem { key1: "foo", key2: b"foo", key3: Path::new("foo") };
    let item2 =
        BorrowedItem { key1: "bar", key2: b"bar", key3: Path::new("bar") };

    // Insert items.
    map.insert_unique(item1.clone()).unwrap();
    map.insert_unique(item2.clone()).unwrap();

    // Check that we can retrieve them.
    assert_eq!(map.get("foo").unwrap().key1, "foo");
    assert_eq!(map.get("bar").unwrap().key1, "bar");

    // Check that we can iterate over them.
    let keys: Vec<_> = map.iter().map(|item| item.key()).collect();
    assert_eq!(keys, vec!["foo", "bar"]);

    // Check that we can print a Debug representation, even within a function
    // (supporting this requires a little bit of unsafe code to get the
    // lifetimes to line up).
    fn fmt_debug(
        map: &IdHashMap<BorrowedItem<'_>, HashBuilder, Alloc>,
    ) -> String {
        format!("{:?}", map)
    }

    static DEBUG_OUTPUT: &str = "{\"foo\": BorrowedItem { \
        key1: \"foo\", key2: [102, 111, 111], key3: \"foo\" }, \
        \"bar\": BorrowedItem { \
        key1: \"bar\", key2: [98, 97, 114], key3: \"bar\" }}";
    assert_eq!(format!("{:?}", map), DEBUG_OUTPUT);
    assert_eq!(fmt_debug(&map), DEBUG_OUTPUT);
}

mod macro_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct User {
        id: u32,
        name: String,
    }

    impl IdHashItem for User {
        type Key<'a> = u32;
        fn key(&self) -> Self::Key<'_> {
            self.id
        }
        id_upcast!();
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_basic() {
        let map = id_hash_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 2, name: "Bob".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&1).unwrap().name, "Alice");
        assert_eq!(map.get(&2).unwrap().name, "Bob");
    }

    #[test]
    fn macro_with_hasher() {
        let map = id_hash_map! {
            HashBuilder;
            User { id: 3, name: "Charlie".to_string() },
            User { id: 4, name: "David".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&3).unwrap().name, "Charlie");
        assert_eq!(map.get(&4).unwrap().name, "David");
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_empty() {
        let empty_map: IdHashMap<User> = id_hash_map! {};
        assert!(empty_map.is_empty());
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_without_trailing_comma() {
        let map = id_hash_map! {
            User { id: 1, name: "Alice".to_string() }
        };
        assert_eq!(map.len(), 1);
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key() {
        let _map = id_hash_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 1, name: "Bob".to_string() },
        };
    }
}

#[cfg(feature = "proptest")]
#[proptest(cases = 16)]
fn proptest_arbitrary_map(map: IdHashMap<TestItem, HashBuilder, Alloc>) {
    // Test that the arbitrarily generated map is valid.
    map.validate(ValidateCompact::NonCompact).expect("map should be valid");

    // Test that we can perform basic operations on the generated map.
    let len = map.len();
    assert_eq!(map.is_empty(), len == 0);

    // Test that we can iterate over the map.
    let mut count = 0;
    for item in &map {
        count += 1;
        // Each item should be findable by its key.
        assert_eq!(map.get(&item.key()), Some(item));
    }
    assert_eq!(count, len);
}

#[cfg(feature = "serde")]
mod serde_tests {
    use iddqd::IdHashMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip,
        test_item::{Alloc, HashBuilder, TestItem},
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestItem>) {
        assert_serialize_roundtrip::<IdHashMap<TestItem, HashBuilder, Alloc>>(
            values,
        );
    }
}
