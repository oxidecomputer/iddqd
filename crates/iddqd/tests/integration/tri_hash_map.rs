use iddqd::{
    TriHashItem, TriHashMap, internal::ValidateCompact, tri_hash_map,
    tri_upcast,
};
use iddqd_test_utils::{
    borrowed_item::BorrowedItem,
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_item::{
        Alloc, HashBuilder, ItemMap, TestItem, TestKey1, TestKey2, TestKey3,
        assert_iter_eq, test_item_permutation_strategy,
    },
};
use proptest::prelude::*;
use std::path::Path;
use test_strategy::{Arbitrary, proptest};

#[derive(Clone, Debug)]
struct SimpleItem {
    key1: u32,
    key2: char,
    key3: u8,
}

impl TriHashItem for SimpleItem {
    type K1<'a> = u32;
    type K2<'a> = char;
    type K3<'a> = u8;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    fn key3(&self) -> Self::K3<'_> {
        self.key3
    }

    tri_upcast!();
}

#[test]
fn debug_impls() {
    let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(SimpleItem { key1: 1, key2: 'a', key3: 0 }).unwrap();
    map.insert_unique(SimpleItem { key1: 20, key2: 'b', key3: 1 }).unwrap();
    map.insert_unique(SimpleItem { key1: 10, key2: 'c', key3: 2 }).unwrap();

    assert_eq!(
        format!("{map:?}"),
        // This is a small-enough map that the order of iteration is
        // deterministic.
        "{{k1: 1, k2: 'a', k3: 0}: SimpleItem { key1: 1, key2: 'a', key3: 0 }, \
          {k1: 10, k2: 'c', k3: 2}: SimpleItem { key1: 10, key2: 'c', key3: 2 }, \
          {k1: 20, k2: 'b', k3: 1}: SimpleItem { key1: 20, key2: 'b', key3: 1 }}",
    );
    assert_eq!(
        format!("{:?}", map.get1_mut(&1).unwrap()),
        "SimpleItem { key1: 1, key2: 'a', key3: 0 }"
    );
}

#[test]
fn test_extend() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let items = vec![
        TestItem::new(1, 'a', "x", "v"),
        TestItem::new(2, 'b', "y", "w"),
        TestItem::new(1, 'c', "z", "overwrote key1"),
        TestItem::new(3, 'b', "q", "overwrote key2"),
        TestItem::new(4, 'd', "x", "overwrote key3"),
        TestItem::new(10, 'A', "X", ""),
        TestItem::new(20, 'B', "Y", ""),
        TestItem::new(30, 'A', "Y", "overwrote key2 and key3"),
        TestItem::new(40, 'C', "Z", ""),
        TestItem::new(50, 'D', "foo", "stays as is"),
        TestItem::new(40, 'E', "Z", "overwrote key1 and key3"),
    ];
    map.extend(items.clone());
    assert_eq!(map.len(), 6);
    assert_eq!(map.get1(&TestKey1::new(&1)).unwrap().value, "overwrote key1");
    assert_eq!(map.get1(&TestKey1::new(&2)), None);
    assert_eq!(map.get1(&TestKey1::new(&3)).unwrap().value, "overwrote key2");
    assert_eq!(map.get1(&TestKey1::new(&4)).unwrap().value, "overwrote key3");
    assert_eq!(
        map.get1(&TestKey1::new(&30)).unwrap().value,
        "overwrote key2 and key3"
    );
    assert_eq!(
        map.get1(&TestKey1::new(&40)).unwrap().value,
        "overwrote key1 and key3"
    );
    assert_eq!(map.get1(&TestKey1::new(&50)).unwrap().value, "stays as is");
}

#[test]
fn with_capacity() {
    let map = TriHashMap::<TestItem, HashBuilder>::with_capacity_and_hasher(
        1024,
        HashBuilder::default(),
    );
    assert!(map.capacity() >= 1024);
}

#[test]
fn test_insert_unique() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add an element.
    let v1 = TestItem::new(0, 'a', "x", "v");
    map.insert_unique(v1.clone()).unwrap();

    // Add an exact duplicate, which should error out.
    let error = map.insert_unique(v1.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v1);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key1, which should error out.
    let v2 = TestItem::new(0, 'b', "y", "v");
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key2, which should error out.
    let v3 = TestItem::new(1, 'a', "y", "v");
    let error = map.insert_unique(v3.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v3);

    // Add a duplicate against just key3, which should error out.
    let v4 = TestItem::new(1, 'b', "x", "v");
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v4);

    // Add an item that doesn't have any conflicts.
    let v5 = TestItem::new(1, 'b', "y", "v");
    map.insert_unique(v5.clone()).unwrap();

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    {
        let mut items: Vec<tri_hash_map::RefMut<_, HashBuilder>> =
            map.iter_mut().collect();
        items.sort_by(|a, b| a.key1().cmp(&b.key1()));
        let e1 = &items[0];
        assert_eq!(**e1, v1);

        // Test that the RefMut Debug impl looks good.
        assert!(
            format!("{:?}", e1).starts_with(
                r#"TestItem { key1: 0, key2: 'a', key3: "x", value: "v""#
            ),
            "RefMut Debug impl should forward to TestItem",
        );

        let e2 = &*items[1];
        assert_eq!(*e2, v5);
    }

    // Check that the *unique methods work.
    assert!(map.contains_key_unique(&v5.key1(), &v5.key2(), &v5.key3()));
    assert_eq!(map.get_unique(&v5.key1(), &v5.key2(), &v5.key3()), Some(&v5));
    assert_eq!(
        *map.get_mut_unique(&v5.key1(), &v5.key2(), &v5.key3()).unwrap(),
        &v5
    );
    assert_eq!(map.remove_unique(&v5.key1(), &v5.key2(), &v5.key3()), Some(v5));
}

// Example-based test for insert_overwrite.
//
// Can be used to write down examples seen from the property-based operation
// test, for easier debugging.
#[test]
fn test_insert_overwrite() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add an element.
    let v1 = TestItem::new(20, 'a', "x", "v");
    assert_eq!(map.insert_overwrite(v1.clone()), Vec::<TestItem>::new());

    // Add an element with the same keys but a different value.
    let v2 = TestItem::new(20, 'a', "x", "w");
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
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
                let map_res = map.get1(&TestKey1::new(&key1));
                let naive_res = naive_map.get1(key1);

                assert_eq!(map_res, naive_res);
            }
            Operation::Get2(key2) => {
                let map_res = map.get2(&TestKey2::new(key2));
                let naive_res = naive_map.get2(key2);

                assert_eq!(map_res, naive_res);
            }
            Operation::Get3(key3) => {
                let map_res = map.get3(&TestKey3::new(&key3));
                let naive_res = naive_map.get3(&key3);

                assert_eq!(map_res, naive_res);
            }
            Operation::Remove1(key1) => {
                let map_res = map.remove1(&TestKey1::new(&key1));
                let naive_res = naive_map.remove1(key1);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Remove2(key2) => {
                let map_res = map.remove2(&TestKey2::new(key2));
                let naive_res = naive_map.remove2(key2);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Remove3(key3) => {
                let map_res = map.remove3(&TestKey3::new(&key3));
                let naive_res = naive_map.remove3(&key3);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
        }

        // Check that the iterators work correctly.
        let mut naive_items = naive_map.iter().collect::<Vec<_>>();
        naive_items.sort_by(|a, b| a.key1().cmp(&b.key1()));

        assert_iter_eq(map.clone(), naive_items);
    }
}

#[proptest(cases = 64)]
fn proptest_permutation_eq(
    #[strategy(test_item_permutation_strategy::<TriHashMap<TestItem, HashBuilder, Alloc>>(0..256))]
    items: (Vec<TestItem>, Vec<TestItem>),
) {
    let (items1, items2) = items;
    let mut map1 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

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
    let mut map1 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

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
        // Insert an item with the same key1 and key3 but a different
        // key2.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(1, 'c', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with the same key1 and key2 but a different
        // key3.
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
#[should_panic(expected = "key1 changed during RefMut borrow")]
fn get_mut_panics_if_key1_changes() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get1_mut(&TestKey1::new(&128)).unwrap().key1 = 2;
}

#[test]
#[should_panic(expected = "key2 changed during RefMut borrow")]
fn get_mut_panics_if_key2_changes() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get1_mut(&TestKey1::new(&128)).unwrap().key2 = 'c';
}

#[test]
#[should_panic(expected = "key3 changed during RefMut borrow")]
fn get_mut_panics_if_key3_changes() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get1_mut(&TestKey1::new(&128)).unwrap().key3 = "z".to_owned();
}

#[test]
fn borrowed_item() {
    let mut map = TriHashMap::<BorrowedItem, HashBuilder, Alloc>::default();
    let item1 =
        BorrowedItem { key1: "foo", key2: b"foo", key3: &Path::new("foo") };
    let item2 =
        BorrowedItem { key1: "bar", key2: b"bar", key3: &Path::new("bar") };

    // Insert items.
    map.insert_unique(item1.clone()).unwrap();
    map.insert_unique(item2.clone()).unwrap();

    // Check that we can retrieve them.
    assert_eq!(map.get1("foo").unwrap().key1, "foo");
    assert_eq!(map.get1("bar").unwrap().key1, "bar");

    // Check that we can iterate over them.
    let keys: Vec<_> = map.iter().map(|item| item.key1()).collect();
    assert_eq!(keys, vec!["foo", "bar"]);

    // Check that we can print a Debug representation, even within a function
    // (supporting this requires a little bit of unsafe code to get the
    // lifetimes to line up).
    fn fmt_debug(
        map: &TriHashMap<BorrowedItem<'_>, HashBuilder, Alloc>,
    ) -> String {
        format!("{:?}", map)
    }

    static DEBUG_OUTPUT: &str = "{{k1: \"foo\", k2: [102, 111, 111], k3: \"foo\"}: BorrowedItem { \
        key1: \"foo\", key2: [102, 111, 111], key3: \"foo\" }, \
        {k1: \"bar\", k2: [98, 97, 114], k3: \"bar\"}: BorrowedItem { \
        key1: \"bar\", key2: [98, 97, 114], key3: \"bar\" }}";

    assert_eq!(format!("{:?}", map), DEBUG_OUTPUT);
    assert_eq!(fmt_debug(&map), DEBUG_OUTPUT);
}

mod macro_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Person {
        id: u32,
        name: String,
        email: String,
    }

    impl TriHashItem for Person {
        type K1<'a> = u32;
        type K2<'a> = &'a str;
        type K3<'a> = &'a str;
        fn key1(&self) -> Self::K1<'_> {
            self.id
        }
        fn key2(&self) -> Self::K2<'_> {
            &self.name
        }
        fn key3(&self) -> Self::K3<'_> {
            &self.email
        }
        tri_upcast!();
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_basic() {
        let map = tri_hash_map! {
            Person { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() },
            Person { id: 2, name: "Bob".to_string(), email: "bob@example.com".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get1(&1).unwrap().name, "Alice");
        assert_eq!(map.get2("Bob").unwrap().id, 2);
        assert_eq!(map.get3("alice@example.com").unwrap().name, "Alice");
    }

    #[test]
    fn macro_with_hasher() {
        let map = tri_hash_map! {
            HashBuilder;
            Person { id: 3, name: "Charlie".to_string(), email: "charlie@example.com".to_string() },
            Person { id: 4, name: "David".to_string(), email: "david@example.com".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get1(&3).unwrap().name, "Charlie");
        assert_eq!(map.get2("David").unwrap().id, 4);
        assert_eq!(map.get3("charlie@example.com").unwrap().name, "Charlie");
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_empty() {
        let empty_map: TriHashMap<Person> = tri_hash_map! {};
        assert!(empty_map.is_empty());
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_without_trailing_comma() {
        let map = tri_hash_map! {
            Person { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() }
        };
        assert_eq!(map.len(), 1);
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key1() {
        let _map = tri_hash_map! {
            Person { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() },
            Person { id: 1, name: "Bob".to_string(), email: "bob@example.com".to_string() },
        };
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key2() {
        let _map = tri_hash_map! {
            Person { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() },
            Person { id: 2, name: "Alice".to_string(), email: "alice2@example.com".to_string() },
        };
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key3() {
        let _map = tri_hash_map! {
            Person { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() },
            Person { id: 2, name: "Bob".to_string(), email: "alice@example.com".to_string() },
        };
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use iddqd::TriHashMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip,
        test_item::{Alloc, HashBuilder, TestItem},
    };
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestItem>) {
        assert_serialize_roundtrip::<TriHashMap<TestItem, HashBuilder, Alloc>>(
            values,
        );
    }
}
