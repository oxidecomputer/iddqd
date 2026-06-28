use crate::hegel_support::{
    draw_fill_batch, draw_lookup_key1, draw_lookup_key2, draw_lookup_key3,
    draw_lookup_keys123, draw_shuffle, test_item,
};
use hegel::{TestCase, generators as gs};
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
        assert_iter_eq,
    },
};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

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
        // Iteration is in insertion order.
        "{{k1: 1, k2: 'a', k3: 0}: SimpleItem { key1: 1, key2: 'a', key3: 0 }, \
          {k1: 20, k2: 'b', k3: 1}: SimpleItem { key1: 20, key2: 'b', key3: 1 }, \
          {k1: 10, k2: 'c', k3: 2}: SimpleItem { key1: 10, key2: 'c', key3: 2 }}",
    );
    assert_eq!(
        format!("{:?}", map.get1_mut(&1).unwrap()),
        "SimpleItem { key1: 1, key2: 'a', key3: 0 }"
    );
}

#[test]
fn debug_impls_borrowed() {
    let before = tri_hash_map! {
        HashBuilder;
        BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
        BorrowedItem { key1: "b", key2: Cow::Borrowed(b"b1"), key3: Path::new("path1") },
        BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b2"), key3: Path::new("path2") },
    };

    assert_eq!(
        format!("{before:?}"),
        r#"{{k1: "a", k2: [98, 48], k3: "path0"}: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, {k1: "b", k2: [98, 49], k3: "path1"}: BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }, {k1: "c", k2: [98, 50], k3: "path2"}: BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }}"#
    );

    #[cfg(feature = "daft")]
    {
        use daft::Diffable;

        let after = tri_hash_map! {
            HashBuilder;
            BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
            BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b3"), key3: Path::new("path3") },
            BorrowedItem { key1: "d", key2: Cow::Borrowed(b"b4"), key3: Path::new("path4") },
        };

        let diff = before.diff(&after).by_unique();
        assert_eq!(
            format!("{diff:?}"),
            r#"Diff { common: {{k1: "a", k2: [98, 48], k3: "path0"}: IdLeaf { before: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, after: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" } }}, added: {{k1: "c", k2: [98, 51], k3: "path3"}: BorrowedItem { key1: "c", key2: [98, 51], key3: "path3" }, {k1: "d", k2: [98, 52], k3: "path4"}: BorrowedItem { key1: "d", key2: [98, 52], key3: "path4" }}, removed: {{k1: "b", k2: [98, 49], k3: "path1"}: BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }, {k1: "c", k2: [98, 50], k3: "path2"}: BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }} }"#
        );
    }
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
            format!("{e1:?}").starts_with(
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

// Focused example-based coverage for the TriHashMap Entry API.
// These tests complement the Hegel state-machine tests below by checking
// public API shape, per-key non-unique mapping, deterministic visit order,
// laziness, and panic-before-mutation behavior.
mod entry_api {
    use super::{
        Alloc, HashBuilder, SimpleItem, TestItem, TestKey1, TestKey2, TestKey3,
    };
    use iddqd::{
        TriHashItem, TriHashMap, internal::ValidateCompact, tri_hash_map,
        tri_upcast,
    };
    use iddqd_test_utils::test_item::ItemMap;

    #[test]
    fn entry_vacant_insert_and_insert_entry() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();

        match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => {
                let inserted =
                    entry.insert(SimpleItem { key1: 1, key2: 'a', key3: 1 });
                assert_eq!(inserted.key1, 1);
            }
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        }
        assert_eq!(map.len(), 1);

        match map.entry(2, 'b', 2) {
            tri_hash_map::Entry::Vacant(entry) => {
                let occupied = entry.insert_entry(SimpleItem {
                    key1: 2,
                    key2: 'b',
                    key3: 2,
                });
                assert!(occupied.is_unique());
                assert_eq!(occupied.into_ref().as_unique().unwrap().key1, 2);
            }
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        }
    }

    #[test]
    fn entry_classifies_unique_partial_and_mixed_lookups() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        map.insert_unique(SimpleItem { key1: 1, key2: 'a', key3: 1 }).unwrap();
        map.insert_unique(SimpleItem { key1: 2, key2: 'b', key3: 2 }).unwrap();
        map.insert_unique(SimpleItem { key1: 3, key2: 'c', key3: 3 }).unwrap();

        match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Occupied(entry) => assert!(entry.is_unique()),
            tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
        }
        for (key1, key2, key3) in [
            (1, 'a', 9),
            (1, 'z', 1),
            (9, 'a', 1),
            (1, 'a', 2),
            (1, 'b', 1),
            (1, 'b', 3),
        ] {
            match map.entry(key1, key2, key3) {
                tri_hash_map::Entry::Occupied(entry) => {
                    assert!(entry.is_non_unique())
                }
                tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
            }
        }
    }

    #[test]
    fn entry_shared_access_preserves_mapping_and_distinct_order() {
        let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

        let a = TestItem::new(1, 'a', "x", "a");
        let b = TestItem::new(2, 'b', "y", "b");
        let c = TestItem::new(3, 'c', "z", "c");

        map.insert_unique(a.clone()).unwrap();
        map.insert_unique(b.clone()).unwrap();
        map.insert_unique(c.clone()).unwrap();

        for (keys, expected_by_key, expected_visit_order) in [
            // A / A / B: key1 and key2 match A, key3 matches B.
            ((1, 'a', "y"), [Some(&a), Some(&a), Some(&b)], vec![&a, &b]),
            // A / A / None: key1 and key2 match A, key3 is absent.
            ((1, 'a', "missing"), [Some(&a), Some(&a), None], vec![&a]),
            // A / None / A: key1 and key3 match A, key2 is absent.
            ((1, 'q', "x"), [Some(&a), None, Some(&a)], vec![&a]),
            // None / A / A: key2 and key3 match A, key1 is absent.
            ((99, 'a', "x"), [None, Some(&a), Some(&a)], vec![&a]),
            // A / B / A: repeated non-unique mapping promised by the PR body.
            ((1, 'b', "x"), [Some(&a), Some(&b), Some(&a)], vec![&a, &b]),
            // A / B / C: all three keys match distinct items.
            ((1, 'b', "z"), [Some(&a), Some(&b), Some(&c)], vec![&a, &b, &c]),
            // None / B / A: visit order is first-key-hit order, not item order.
            ((99, 'b', "x"), [None, Some(&b), Some(&a)], vec![&b, &a]),
        ] {
            let entry_ref = match map.entry(
                TestKey1::new(&keys.0),
                TestKey2::new(keys.1),
                TestKey3::new(keys.2),
            ) {
                tri_hash_map::Entry::Occupied(entry) => {
                    assert!(entry.is_non_unique());
                    entry.into_ref()
                }
                tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
            };

            assert!(entry_ref.is_non_unique());
            assert_eq!(entry_ref.by_key1(), expected_by_key[0]);
            assert_eq!(entry_ref.by_key2(), expected_by_key[1]);
            assert_eq!(entry_ref.by_key3(), expected_by_key[2]);

            let tri_hash_map::OccupiedEntryRef::NonUnique(non_unique) =
                entry_ref
            else {
                panic!("expected non-unique entry ref");
            };

            assert_eq!(non_unique.by_key1(), expected_by_key[0]);
            assert_eq!(non_unique.by_key2(), expected_by_key[1]);
            assert_eq!(non_unique.by_key3(), expected_by_key[2]);

            let mut seen = Vec::new();
            non_unique.for_each(|item| seen.push(item));
            assert_eq!(seen, expected_visit_order);
        }
    }

    #[test]
    #[should_panic(expected = "key1 hashes do not match")]
    fn entry_vacant_insert_panics_on_mismatched_key1() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert(SimpleItem { key1: 2, key2: 'a', key3: 1 });
    }

    #[test]
    #[should_panic(expected = "key1 hashes do not match")]
    fn entry_vacant_insert_entry_panics_on_mismatched_key1() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert_entry(SimpleItem { key1: 2, key2: 'a', key3: 1 });
    }

    #[test]
    #[should_panic(expected = "key2 hashes do not match")]
    fn entry_vacant_insert_panics_on_mismatched_key2() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert(SimpleItem { key1: 1, key2: 'b', key3: 1 });
    }

    #[test]
    #[should_panic(expected = "key2 hashes do not match")]
    fn entry_vacant_insert_entry_panics_on_mismatched_key2() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert_entry(SimpleItem { key1: 1, key2: 'b', key3: 1 });
    }

    #[test]
    #[should_panic(expected = "key3 hashes do not match")]
    fn entry_vacant_insert_panics_on_mismatched_key3() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert(SimpleItem { key1: 1, key2: 'a', key3: 2 });
    }

    #[test]
    #[should_panic(expected = "key3 hashes do not match")]
    fn entry_vacant_insert_entry_panics_on_mismatched_key3() {
        let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
        let entry = match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Vacant(entry) => entry,
            tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
        };
        entry.insert_entry(SimpleItem { key1: 1, key2: 'a', key3: 2 });
    }

    #[derive(Clone, Debug)]
    struct EntryMutItem {
        key1: u32,
        key2: char,
        key3: u8,
        value: &'static str,
    }

    impl TriHashItem for EntryMutItem {
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

    fn entry_mut_map() -> TriHashMap<EntryMutItem, HashBuilder, Alloc> {
        let mut map =
            TriHashMap::<EntryMutItem, HashBuilder, Alloc>::make_new();
        map.insert_unique(EntryMutItem {
            key1: 1,
            key2: 'a',
            key3: 1,
            value: "A",
        })
        .unwrap();
        map.insert_unique(EntryMutItem {
            key1: 2,
            key2: 'b',
            key3: 2,
            value: "B",
        })
        .unwrap();
        map.insert_unique(EntryMutItem {
            key1: 3,
            key2: 'c',
            key3: 3,
            value: "C",
        })
        .unwrap();
        map
    }

    fn removed_values(items: Vec<EntryMutItem>) -> Vec<&'static str> {
        items.into_iter().map(|item| item.value).collect()
    }

    #[test]
    fn entry_mut_unique_and_non_unique_accessors_preserve_mapping() {
        let mut map = entry_mut_map();
        match map.entry(1, 'a', 1) {
            tri_hash_map::Entry::Occupied(mut entry) => {
                let mut view = entry.get_mut();
                assert!(view.is_unique());
                view.as_unique().unwrap().value = "unique";
            }
            tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
        }
        assert_eq!(map.get1(&1).unwrap().value, "unique");

        for (keys, expected) in [
            ((1, 'a', 9), [Some("A"), Some("A"), None]),
            ((1, 'z', 1), [Some("A"), None, Some("A")]),
            ((9, 'a', 1), [None, Some("A"), Some("A")]),
            ((1, 'a', 2), [Some("A"), Some("A"), Some("B")]),
            ((1, 'b', 1), [Some("A"), Some("B"), Some("A")]),
            ((1, 'b', 3), [Some("A"), Some("B"), Some("C")]),
        ] {
            let mut map = entry_mut_map();
            match map.entry(keys.0, keys.1, keys.2) {
                tri_hash_map::Entry::Occupied(mut entry) => {
                    let mut view = entry.get_mut();
                    assert!(view.is_non_unique());
                    assert_eq!(
                        view.by_key1().map(|item| item.value),
                        expected[0]
                    );
                    assert_eq!(
                        view.by_key2().map(|item| item.value),
                        expected[1]
                    );
                    assert_eq!(
                        view.by_key3().map(|item| item.value),
                        expected[2]
                    );
                }
                tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
            }
        }
    }

    #[test]
    fn entry_mut_sequential_overlapping_access_and_for_each_order() {
        let mut map = entry_mut_map();
        match map.entry(1, 'a', 9) {
            tri_hash_map::Entry::Occupied(mut entry) => {
                let mut view = entry.get_mut();
                view.by_key1().unwrap().value = "first";
                view.by_key2().unwrap().value = "second";
                assert!(view.by_key3().is_none());
                let mut seen = Vec::new();
                view.for_each(|mut item| {
                    seen.push(item.value);
                    item.value = "done";
                });
                assert_eq!(seen, vec!["second"]);
            }
            tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
        }
        assert_eq!(map.get1(&1).unwrap().value, "done");

        for (keys, expected) in [
            ((1, 'z', 1), vec!["A"]),
            ((9, 'a', 1), vec!["A"]),
            ((1, 'a', 2), vec!["A", "B"]),
            ((1, 'b', 1), vec!["A", "B"]),
            ((1, 'b', 3), vec!["A", "B", "C"]),
        ] {
            let mut map = entry_mut_map();
            match map.entry(keys.0, keys.1, keys.2) {
                tri_hash_map::Entry::Occupied(mut entry) => {
                    let mut seen = Vec::new();
                    entry.get_mut().for_each(|item| seen.push(item.value));
                    assert_eq!(seen, expected);
                }
                tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
            }
        }
    }

    #[test]
    fn entry_and_modify_visits_distinct_occupied_items_only() {
        let mut map = entry_mut_map();
        map.entry(1, 'a', 9).and_modify(|mut item| item.value = "AA");
        assert_eq!(map.get1(&1).unwrap().value, "AA");

        map.entry(1, 'a', 2).and_modify(|mut item| {
            item.value = match item.value {
                "AA" => "A2",
                "B" => "B2",
                other => other,
            };
        });
        assert_eq!(map.get1(&1).unwrap().value, "A2");
        assert_eq!(map.get1(&2).unwrap().value, "B2");

        let mut seen = Vec::new();
        map.entry(1, 'b', 3).and_modify(|item| seen.push(item.value));
        assert_eq!(seen, vec!["A2", "B2", "C"]);

        map.entry(9, 'z', 9).and_modify(|mut item| item.value = "vacant");
        assert_eq!(map.len(), 3);
        assert!(map.get1(&9).is_none());
    }

    #[test]
    fn entry_remove_returns_first_key_hit_order_and_removes_once() {
        for (keys, expected) in [
            ((1, 'a', 1), vec!["A"]),
            ((1, 'a', 9), vec!["A"]),
            ((1, 'z', 1), vec!["A"]),
            ((9, 'a', 1), vec!["A"]),
            ((1, 'a', 2), vec!["A", "B"]),
            ((1, 'b', 1), vec!["A", "B"]),
            ((1, 'b', 3), vec!["A", "B", "C"]),
            ((9, 'b', 1), vec!["B", "A"]),
        ] {
            let mut map = entry_mut_map();
            let removed = match map.entry(keys.0, keys.1, keys.2) {
                tri_hash_map::Entry::Occupied(entry) => entry.remove(),
                tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
            };
            assert_eq!(removed_values(removed), expected);
            assert!(map.validate(ValidateCompact::NonCompact).is_ok());
        }
    }

    #[test]
    fn entry_insert_replaces_first_key_hit_order_and_becomes_unique() {
        for (keys, expected) in [
            ((1, 'a', 1), vec!["A"]),
            ((1, 'a', 9), vec!["A"]),
            ((1, 'z', 1), vec!["A"]),
            ((9, 'a', 1), vec!["A"]),
            ((1, 'a', 2), vec!["A", "B"]),
            ((1, 'b', 1), vec!["A", "B"]),
            ((1, 'b', 3), vec!["A", "B", "C"]),
            ((9, 'b', 1), vec!["B", "A"]),
        ] {
            let mut map = entry_mut_map();
            {
                let mut entry = match map.entry(keys.0, keys.1, keys.2) {
                    tri_hash_map::Entry::Occupied(entry) => entry,
                    tri_hash_map::Entry::Vacant(_) => {
                        panic!("expected occupied")
                    }
                };
                let removed = entry.insert(EntryMutItem {
                    key1: keys.0,
                    key2: keys.1,
                    key3: keys.2,
                    value: "R",
                });
                assert_eq!(removed_values(removed), expected);
                assert!(entry.is_unique());
                assert_eq!(entry.get().as_unique().unwrap().value, "R");
            }
            assert_eq!(map.get1(&keys.0).unwrap().value, "R");
            assert_eq!(map.get2(&keys.1).unwrap().value, "R");
            assert_eq!(map.get3(&keys.2).unwrap().value, "R");
            assert!(map.validate(ValidateCompact::NonCompact).is_ok());
        }
    }

    #[test]
    fn entry_or_insert_only_inserts_vacant_and_or_insert_with_is_lazy() {
        let mut map = entry_mut_map();
        let mut inserted = map.entry(4, 'd', 4).or_insert(EntryMutItem {
            key1: 4,
            key2: 'd',
            key3: 4,
            value: "D",
        });
        assert_eq!(inserted.as_unique().unwrap().value, "D");
        drop(inserted);
        assert_eq!(map.len(), 4);

        for (keys, is_unique) in [
            ((1, 'a', 1), true),
            ((1, 'a', 9), false),
            ((1, 'z', 1), false),
            ((9, 'a', 1), false),
            ((1, 'a', 2), false),
            ((1, 'b', 1), false),
            ((1, 'b', 3), false),
        ] {
            let before = map.len();
            let view = map.entry(keys.0, keys.1, keys.2).or_insert_with(|| {
                panic!("or_insert_with called for occupied entry")
            });

            assert_eq!(view.is_unique(), is_unique);
            assert_eq!(view.is_non_unique(), !is_unique);

            drop(view);
            assert_eq!(map.len(), before);
        }
    }

    #[test]
    fn entry_insert_panics_before_mutating_map() {
        for bad in [
            EntryMutItem { key1: 9, key2: 'a', key3: 2, value: "bad-k1" },
            EntryMutItem { key1: 1, key2: 'z', key3: 2, value: "bad-k2" },
            EntryMutItem { key1: 1, key2: 'a', key3: 9, value: "bad-k3" },
            EntryMutItem { key1: 1, key2: 'a', key3: 1, value: "bad-dup" },
        ] {
            let mut map = entry_mut_map();
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut entry = match map.entry(1, 'a', 2) {
                        tri_hash_map::Entry::Occupied(entry) => entry,
                        tri_hash_map::Entry::Vacant(_) => {
                            panic!("expected occupied")
                        }
                    };
                    entry.insert(bad);
                }));
            assert!(result.is_err());
            assert_eq!(map.len(), 3);
            assert_eq!(map.get1(&1).unwrap().value, "A");
            assert_eq!(map.get1(&2).unwrap().value, "B");
            assert!(map.validate(ValidateCompact::NonCompact).is_ok());
        }
    }
}

// Test that the unsafe block within RefMut doesn't trip up miri.
#[test]
fn test_ref_mut_aliasing() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    for i in 0..16_u8 {
        let key2 = (b'a' + i) as char;
        let key3 = format!("k{i}");
        map.insert_unique(TestItem::new(i, key2, key3, "v")).unwrap();
    }

    let mut items: Vec<_> = map.iter_mut().collect();
    for (i, item) in items.iter_mut().enumerate() {
        item.value = format!("written-{i}");
    }
    drop(items);

    for i in 0..16_u8 {
        let item = map.get1(&TestKey1::new(&i)).unwrap();
        assert!(item.value.starts_with("written-"));
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactnessChange {
    /// The operation makes the map non-compact.
    NoLongerCompact,
    /// The operation makes the map compact.
    BecomesCompact,
    /// The operation doesn't change compactness.
    NoChange,
}

impl CompactnessChange {
    /// Applies this compactness change to the given compactness state.
    fn apply(self, compactness: ValidateCompact) -> ValidateCompact {
        match (compactness, self) {
            (ValidateCompact::Compact, CompactnessChange::NoLongerCompact) => {
                ValidateCompact::NonCompact
            }
            (
                ValidateCompact::NonCompact,
                CompactnessChange::BecomesCompact,
            ) => ValidateCompact::Compact,
            _ => compactness,
        }
    }
}

struct TriHashMapMachine {
    map: TriHashMap<TestItem, HashBuilder, Alloc>,
    naive: NaiveMap,
    compactness: ValidateCompact,
}

impl TriHashMapMachine {
    fn check_valid(&mut self, change: CompactnessChange) {
        self.compactness = change.apply(self.compactness);
        self.map.validate(self.compactness).expect("map should be valid");
    }
}

#[hegel::state_machine]
impl TriHashMapMachine {
    #[rule]
    fn insert_unique(&mut self, tc: TestCase) {
        let item = tc.draw(test_item());
        let map_res = self.map.insert_unique(item.clone());
        let naive_res = self.naive.insert_unique(item.clone());

        assert_eq!(
            map_res.is_ok(),
            naive_res.is_ok(),
            "map and naive map should agree on insert result"
        );
        if let Err(map_err) = map_res {
            let naive_err = naive_res.unwrap_err();
            assert_eq!(map_err.new_item(), naive_err.new_item());
            // The duplicates may be in any order, so sort them before
            // comparing.
            let mut map_err_dups = map_err.duplicates().to_vec();
            let mut naive_err_dups = naive_err.duplicates().to_vec();
            map_err_dups.sort();
            naive_err_dups.sort();
            assert_eq!(map_err_dups, naive_err_dups);
        }

        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn insert_overwrite(&mut self, tc: TestCase) {
        let item = tc.draw(test_item());
        let mut map_dups = self.map.insert_overwrite(item.clone());
        map_dups.sort();
        let mut naive_dups = self.naive.insert_overwrite(item.clone());
        naive_dups.sort();

        assert_eq!(
            map_dups, naive_dups,
            "map and naive map should agree on insert_overwrite dups"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn entry_insert_overwrite(&mut self, tc: TestCase) {
        let (key1, key2, key3) = draw_lookup_keys123(&tc, &self.naive);
        let value = tc.draw(gs::text());
        let item = TestItem::new(key1, key2, key3, value);

        let map_res = match self.map.entry(
            TestKey1::new(&item.key1),
            TestKey2::new(item.key2),
            TestKey3::new(&item.key3),
        ) {
            tri_hash_map::Entry::Occupied(mut entry) => {
                Some(entry.insert(item.clone()))
            }
            tri_hash_map::Entry::Vacant(_) => None,
        };

        let naive_res = self.naive.entry_insert_overwrite123(item);

        assert_eq!(
            map_res, naive_res,
            "map and naive map should agree on Entry::insert removed items"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn entry_remove(&mut self, tc: TestCase) {
        let (key1, key2, key3) = draw_lookup_keys123(&tc, &self.naive);

        let map_res = match self.map.entry(
            TestKey1::new(&key1),
            TestKey2::new(key2),
            TestKey3::new(&key3),
        ) {
            tri_hash_map::Entry::Occupied(entry) => entry.remove(),
            tri_hash_map::Entry::Vacant(_) => Vec::new(),
        };

        let naive_res = self.naive.entry_remove123(key1, key2, &key3);

        assert_eq!(
            map_res, naive_res,
            "map and naive map should agree on Entry::remove items"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn get1(&mut self, tc: TestCase) {
        let key1 = draw_lookup_key1(&tc, &self.naive);
        let map_res = self.map.get1(&TestKey1::new(&key1));
        let naive_res = self.naive.get1(key1);

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn get2(&mut self, tc: TestCase) {
        let key2 = draw_lookup_key2(&tc, &self.naive);
        let map_res = self.map.get2(&TestKey2::new(key2));
        let naive_res = self.naive.get2(key2);

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn get3(&mut self, tc: TestCase) {
        let key3 = draw_lookup_key3(&tc, &self.naive);
        let map_res = self.map.get3(&TestKey3::new(&key3));
        let naive_res = self.naive.get3(&key3);

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn get_unique(&mut self, tc: TestCase) {
        let (key1, key2, key3) = draw_lookup_keys123(&tc, &self.naive);
        let map_res = self.map.get_unique(
            &TestKey1::new(&key1),
            &TestKey2::new(key2),
            &TestKey3::new(&key3),
        );
        let naive_res = self.naive.get_unique123(key1, key2, &key3);

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn get_mut_unique(&mut self, tc: TestCase) {
        let (key1, key2, key3) = draw_lookup_keys123(&tc, &self.naive);
        let map_res = self
            .map
            .get_mut_unique(
                &TestKey1::new(&key1),
                &TestKey2::new(key2),
                &TestKey3::new(&key3),
            )
            .map(|r| (*r).clone());
        let naive_res =
            self.naive.get_mut_unique123(key1, key2, &key3).cloned();

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn remove1(&mut self, tc: TestCase) {
        let key1 = draw_lookup_key1(&tc, &self.naive);
        let map_res = self.map.remove1(&TestKey1::new(&key1));
        let naive_res = self.naive.remove1(key1);

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn remove2(&mut self, tc: TestCase) {
        let key2 = draw_lookup_key2(&tc, &self.naive);
        let map_res = self.map.remove2(&TestKey2::new(key2));
        let naive_res = self.naive.remove2(key2);

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn remove3(&mut self, tc: TestCase) {
        let key3 = draw_lookup_key3(&tc, &self.naive);
        let map_res = self.map.remove3(&TestKey3::new(&key3));
        let naive_res = self.naive.remove3(&key3);

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn remove_unique(&mut self, tc: TestCase) {
        let (key1, key2, key3) = draw_lookup_keys123(&tc, &self.naive);
        let map_res = self.map.remove_unique(
            &TestKey1::new(&key1),
            &TestKey2::new(key2),
            &TestKey3::new(&key3),
        );
        let naive_res = self.naive.remove_unique123(key1, key2, &key3);

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn retain_value_contains(&mut self, tc: TestCase) {
        let ch = tc.draw(gs::characters());
        let equals = tc.draw(gs::booleans());
        self.map.retain(|item| {
            let contains = item.value.contains(ch);
            if equals { contains } else { !contains }
        });
        self.naive.retain(|item| {
            let contains = item.value.contains(ch);
            if equals { contains } else { !contains }
        });
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn retain_modulo(&mut self, tc: TestCase) {
        let a = tc.draw(gs::integers::<u8>().max_value(2));
        let b = tc.draw(gs::integers::<u8>().min_value(1).max_value(3));
        let equals = tc.draw(gs::booleans());
        let modulo = a + b;
        let remainder = a;
        self.map.retain(|item| {
            let matches = item.key1 % modulo == remainder;
            if equals { matches } else { !matches }
        });
        self.naive.retain(|item| {
            let matches = item.key1 % modulo == remainder;
            if equals { matches } else { !matches }
        });
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn extend(&mut self, tc: TestCase) {
        let items = tc.draw(gs::vecs(test_item()).max_size(15));
        self.map.extend(items.clone());
        self.naive.extend(items);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    // Fill up the map to ensure later operations use a larger map.
    #[rule]
    fn fill(&mut self, tc: TestCase) {
        let items = draw_fill_batch(&tc);
        self.map.extend(items.clone());
        self.naive.extend(items);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn clear(&mut self, _: TestCase) {
        self.map.clear();
        self.naive.clear();
        self.check_valid(CompactnessChange::BecomesCompact);
    }

    #[rule]
    fn reserve(&mut self, tc: TestCase) {
        let additional = tc.draw(gs::integers::<usize>().max_value(255));
        self.map.reserve(additional);
        // `reserve` has no observable effect beyond capacity -- the
        // naive map has no equivalent. `check_valid` will iterate items
        // and ask `find_index` for each, which catches a hash-table
        // left mis-bucketed by a regrowth rehash.
        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn try_reserve(&mut self, tc: TestCase) {
        let additional = tc.draw(gs::integers::<usize>().max_value(255));
        let _ = self.map.try_reserve(additional);
        // See the comment on `reserve` above for why this is only
        // `check_valid`.
        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn shrink_to_fit(&mut self, _: TestCase) {
        self.map.shrink_to_fit();
        self.check_valid(CompactnessChange::BecomesCompact);
    }

    #[rule]
    fn shrink_to(&mut self, tc: TestCase) {
        let min_capacity = tc.draw(gs::integers::<usize>().max_value(255));
        self.map.shrink_to(min_capacity);
        self.check_valid(CompactnessChange::BecomesCompact);
    }

    #[invariant]
    fn iter_matches(&mut self, _: TestCase) {
        let mut naive_items = self.naive.iter().collect::<Vec<_>>();
        naive_items.sort_by(|a, b| a.key1().cmp(&b.key1()));
        assert_iter_eq(self.map.clone(), naive_items);
    }
}

#[hegel::test(test_cases = 512)]
fn proptest_ops(tc: TestCase) {
    let machine = TriHashMapMachine {
        map: TriHashMap::<TestItem, HashBuilder, Alloc>::make_new(),
        naive: NaiveMap::new_key123(),
        compactness: ValidateCompact::Compact,
    };
    hegel::stateful::run(machine, tc);
}

#[hegel::test(test_cases = 64)]
fn proptest_permutation_eq(tc: TestCase) {
    // draw_fill_batch generates unique keys so there's no need to deduplicate.
    let set = draw_fill_batch(&tc);
    let set2 = draw_shuffle(&tc, &set);

    let mut map1 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    for item in set {
        map1.insert_unique(item).expect("set is deduplicated");
    }
    for item in set2 {
        map2.insert_unique(item).expect("set is deduplicated");
    }

    assert_eq_props(&map1, &map2);
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
    let item1 = BorrowedItem {
        key1: "foo",
        key2: Cow::Borrowed(b"foo"),
        key3: Path::new("foo"),
    };
    let item2 = BorrowedItem {
        key1: "bar",
        key2: Cow::Borrowed(b"bar"),
        key3: Path::new("bar"),
    };

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
        format!("{map:?}")
    }

    #[cfg(feature = "serde")]
    fn serialize_as_map(
        map: &TriHashMap<BorrowedItem<'_>, HashBuilder, Alloc>,
    ) -> Result<String, iddqd_test_utils::serde_json::Error> {
        let mut out: Vec<u8> = Vec::new();
        let mut ser = iddqd_test_utils::serde_json::Serializer::new(&mut out);
        tri_hash_map::TriHashMapAsMap::serialize(map, &mut ser)?;
        Ok(String::from_utf8(out)
            .expect("serde_json should always emit valid UTF-8"))
    }

    static DEBUG_OUTPUT: &str = "{{k1: \"foo\", k2: [102, 111, 111], k3: \"foo\"}: BorrowedItem { \
        key1: \"foo\", key2: [102, 111, 111], key3: \"foo\" }, \
        {k1: \"bar\", k2: [98, 97, 114], k3: \"bar\"}: BorrowedItem { \
        key1: \"bar\", key2: [98, 97, 114], key3: \"bar\" }}";

    assert_eq!(format!("{map:?}"), DEBUG_OUTPUT);
    assert_eq!(fmt_debug(&map), DEBUG_OUTPUT);

    #[cfg(feature = "serde")]
    {
        let map_string = serialize_as_map(&map).unwrap();
        let deserialized: TriHashMap<BorrowedItem<'_>, HashBuilder, Alloc> =
            iddqd_test_utils::serde_json::from_str(&map_string).unwrap();
        assert_eq!(map, deserialized);
    }
}

#[test]
fn borrowed_item_retain_non_static() {
    let foo_key = String::from("foo");
    let bar_key = String::from("bar");
    let foo_bytes = b"foo".to_vec();
    let bar_bytes = b"bar".to_vec();
    let foo_path = PathBuf::from("foo");
    let bar_path = PathBuf::from("bar");

    let mut map = TriHashMap::<BorrowedItem<'_>, HashBuilder, Alloc>::default();
    map.insert_unique(BorrowedItem {
        key1: foo_key.as_str(),
        key2: Cow::Borrowed(foo_bytes.as_slice()),
        key3: foo_path.as_path(),
    })
    .unwrap();
    map.insert_unique(BorrowedItem {
        key1: bar_key.as_str(),
        key2: Cow::Borrowed(bar_bytes.as_slice()),
        key3: bar_path.as_path(),
    })
    .unwrap();

    map.retain(|item| item.key1 == foo_key.as_str());

    assert_eq!(map.len(), 1);
    assert!(map.get1(foo_key.as_str()).is_some());
    assert!(map.get1(bar_key.as_str()).is_none());
}

#[test]
fn test_retain_all() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    let original_len = map.len();
    map.retain(|_| true);

    assert_eq!(map.len(), original_len);
    assert_eq!(map.len(), 3);
    map.get1(&TestKey1::new(&1)).expect("key1=1 should be present");
    map.get1(&TestKey1::new(&2)).expect("key1=2 should be present");
    map.get1(&TestKey1::new(&3)).expect("key1=3 should be present");
}

#[test]
fn test_retain_none() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    map.retain(|_| false);

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn test_retain_value_contains() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();
    map.insert_unique(TestItem::new(4, 'd', "w", "qux")).unwrap();

    map.retain(|item| item.value.contains('a'));

    assert_eq!(map.len(), 2);
    map.get1(&TestKey1::new(&2)).expect("key1=2 (bar) should be present");
    map.get1(&TestKey1::new(&3)).expect("key1=3 (baz) should be present");
    assert!(
        map.get1(&TestKey1::new(&1)).is_none(),
        "key1=1 (foo) should be removed"
    );
    assert!(
        map.get1(&TestKey1::new(&4)).is_none(),
        "key1=4 (qux) should be removed"
    );
}

#[test]
fn test_retain_modulo() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(0, 'a', "x", "v0")).unwrap();
    map.insert_unique(TestItem::new(1, 'b', "y", "v1")).unwrap();
    map.insert_unique(TestItem::new(2, 'c', "z", "v2")).unwrap();
    map.insert_unique(TestItem::new(3, 'd', "w", "v3")).unwrap();
    map.insert_unique(TestItem::new(4, 'e', "u", "v4")).unwrap();
    map.insert_unique(TestItem::new(5, 'f', "t", "v5")).unwrap();

    map.retain(|item| item.key1 % 3 == 1);

    assert_eq!(map.len(), 2);
    map.get1(&TestKey1::new(&1)).expect("key1=1 should be present");
    map.get1(&TestKey1::new(&4)).expect("key1=4 should be present");
    assert!(map.get1(&TestKey1::new(&0)).is_none(), "key1=0 should be removed");
    assert!(map.get1(&TestKey1::new(&2)).is_none(), "key1=2 should be removed");
    assert!(map.get1(&TestKey1::new(&3)).is_none(), "key1=3 should be removed");
    assert!(map.get1(&TestKey1::new(&5)).is_none(), "key1=5 should be removed");

    // Test with a larger map for miri coverage.
    let mut large_map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    for i in 0..32_u8 {
        large_map
            .insert_unique(TestItem::new(
                i,
                char::from(b'a' + i),
                format!("k{}", i),
                "z",
            ))
            .unwrap();
    }

    large_map.retain(|item| item.key1 % 7 == 3);

    for i in 0..32_u8 {
        if i % 7 == 3 {
            large_map
                .get1(&TestKey1::new(&i))
                .unwrap_or_else(|| panic!("key1={} should be present", i));
        } else {
            assert!(
                large_map.get1(&TestKey1::new(&i)).is_none(),
                "key1={} should be removed",
                i
            );
        }
    }
}

#[test]
fn test_retain_empty_map() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.retain(|_| true);
    assert!(map.is_empty());
}

#[test]
fn test_clear_empty_map() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.clear();
    assert!(map.is_empty());
    map.validate(ValidateCompact::Compact)
        .expect("empty cleared map should be compact");
}

#[test]
fn test_clear_makes_compact() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add items
    map.insert_unique(TestItem::new(1, 'a', "x", "v1")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "v2")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "v3")).unwrap();

    // Remove an item to make it non-compact
    map.remove1(&TestKey1::new(&2));
    map.validate(ValidateCompact::NonCompact)
        .expect("map should be valid but non-compact");

    // Clear should make it compact again
    map.clear();
    assert!(map.is_empty());
    map.validate(ValidateCompact::Compact)
        .expect("cleared map should be compact");
}

#[test]
fn test_retain_verifies_all_keys() {
    let mut map = TriHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    // Retain only key1=2
    map.retain(|item| item.key1 == 2);

    // Verify all three keys work
    map.get1(&TestKey1::new(&2)).expect("key1=2 should be present");
    map.get2(&TestKey2::new('b')).expect("key2='b' should be present");
    map.get3(&TestKey3::new("y")).expect("key3=\"y\" should be present");
    assert!(map.get1(&TestKey1::new(&1)).is_none());
    assert!(map.get2(&TestKey2::new('a')).is_none());
    assert!(map.get3(&TestKey3::new("x")).is_none());
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
    use crate::hegel_support::draw_random_batch;
    use hegel::TestCase;
    use iddqd::TriHashMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip,
        test_item::{Alloc, HashBuilder, TestItem},
    };

    #[hegel::test(test_cases = 256)]
    fn proptest_serialize_roundtrip(tc: TestCase) {
        let values = draw_random_batch(&tc);
        assert_serialize_roundtrip::<TriHashMap<TestItem, HashBuilder, Alloc>>(
            values,
        );
    }
}

#[cfg(feature = "proptest")]
use test_strategy::proptest;

#[cfg(feature = "proptest")]
#[proptest(cases = 16)]
fn proptest_arbitrary_map(map: TriHashMap<TestItem, HashBuilder, Alloc>) {
    // Test that the arbitrarily generated map is valid.
    map.validate(ValidateCompact::NonCompact).expect("map should be valid");

    // Test that we can perform basic operations on the generated map.
    let len = map.len();
    assert_eq!(map.is_empty(), len == 0);

    // Test that we can iterate over the map.
    let mut count = 0;
    for item in &map {
        count += 1;
        // Each item should be findable by all three keys.
        assert_eq!(map.get1(&item.key1()), Some(item));
        assert_eq!(map.get2(&item.key2()), Some(item));
        assert_eq!(map.get3(&item.key3()), Some(item));
    }
    assert_eq!(count, len);
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
#[derive(Clone, Debug)]
struct PanickyHashItem {
    key1: u32,
    key2: u32,
    key3: u32,
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
impl TriHashItem for PanickyHashItem {
    type K1<'a> = iddqd_test_utils::panic_safety::PanickyKey;
    type K2<'a> = iddqd_test_utils::panic_safety::PanickyKey;
    type K3<'a> = iddqd_test_utils::panic_safety::PanickyKey;

    fn key1(&self) -> Self::K1<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key1");
        iddqd_test_utils::panic_safety::PanickyKey(self.key1)
    }

    fn key2(&self) -> Self::K2<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key2");
        iddqd_test_utils::panic_safety::PanickyKey(self.key2)
    }

    fn key3(&self) -> Self::K3<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key3");
        iddqd_test_utils::panic_safety::PanickyKey(self.key3)
    }

    tri_upcast!();
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
impl Drop for PanickyHashItem {
    fn drop(&mut self) {
        iddqd_test_utils::panic_safety::observe_panicky_call("item-drop");
    }
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
mod proptest_panic_safety {
    use super::*;
    use crate::hegel_support::{MAX_PANIC_KEY, draw_armed};
    use allocator_api2::alloc::Global;
    use iddqd_test_utils::panic_safety::{
        PanicSafety, PanickyAlloc, PanickySearchKey,
        assert_panic_fired_as_expected, assert_post_op_invariants,
        drop_unarmed, record_observation, run_armed, sorted_keys,
    };

    type PanickyMap = TriHashMap<
        PanickyHashItem,
        iddqd::DefaultHashBuilder,
        PanickyAlloc<Global>,
    >;

    struct PanicMachine {
        map: PanickyMap,
        step: usize,
        pending: Option<Pending>,
    }

    struct Pending {
        label: &'static str,
        panic_safety: PanicSafety,
        armed: Option<u32>,
        panicked: bool,
        pre_state: Vec<(u32, u32, u32)>,
    }

    impl PanicMachine {
        fn armed_op(
            &mut self,
            tc: &TestCase,
            label: &'static str,
            panic_safety: PanicSafety,
            op: impl FnOnce(&mut PanickyMap),
        ) {
            // hegel runs the `#[invariant]` (which consumes `pending`) after
            // every successful rule, so `pending` must be `None` here -- if
            // not, a prior op's post-op checks were silently skipped.
            assert!(
                self.pending.is_none(),
                "previous op's post-op invariant did not run before this op",
            );
            let armed = draw_armed(tc);
            let pre_state = sorted_keys(&self.map, |item| {
                (item.key1, item.key2, item.key3)
            });
            let (panicked, ops) = run_armed(armed, || op(&mut self.map));
            record_observation("tri_hash_map", label, ops);
            assert_panic_fired_as_expected(&label, armed, panicked, ops);

            // `self.pending` is set at the end of this function, after all
            // fallible draws.
            self.pending = Some(Pending {
                label,
                panic_safety,
                armed,
                panicked,
                pre_state,
            });
        }
    }

    #[hegel::state_machine]
    impl PanicMachine {
        #[rule]
        fn insert_unique(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key3 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "insert_unique", PanicSafety::Atomic, |map| {
                drop_unarmed(map.insert_unique(PanickyHashItem {
                    key1,
                    key2,
                    key3,
                }));
            });
        }

        #[rule]
        fn insert_overwrite(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key3 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(
                &tc,
                "insert_overwrite",
                PanicSafety::Atomic,
                |map| {
                    drop_unarmed(map.insert_overwrite(PanickyHashItem {
                        key1,
                        key2,
                        key3,
                    }));
                },
            );
        }

        #[rule]
        fn remove1(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "remove1", PanicSafety::Atomic, |map| {
                drop_unarmed(map.remove1(&PanickySearchKey(key1)));
            });
        }

        #[rule]
        fn remove2(&mut self, tc: TestCase) {
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "remove2", PanicSafety::Atomic, |map| {
                drop_unarmed(map.remove2(&PanickySearchKey(key2)));
            });
        }

        #[rule]
        fn remove3(&mut self, tc: TestCase) {
            let key3 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "remove3", PanicSafety::Atomic, |map| {
                drop_unarmed(map.remove3(&PanickySearchKey(key3)));
            });
        }

        #[rule]
        fn get1(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "get1", PanicSafety::Atomic, |map| {
                let _ = map.get1(&PanickySearchKey(key1));
            });
        }

        #[rule]
        fn get2(&mut self, tc: TestCase) {
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "get2", PanicSafety::Atomic, |map| {
                let _ = map.get2(&PanickySearchKey(key2));
            });
        }

        #[rule]
        fn get3(&mut self, tc: TestCase) {
            let key3 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "get3", PanicSafety::Atomic, |map| {
                let _ = map.get3(&PanickySearchKey(key3));
            });
        }

        #[rule]
        fn retain_modulo(&mut self, tc: TestCase) {
            let rem = tc.draw(gs::integers::<u32>().max_value(2));
            let modulo =
                tc.draw(gs::integers::<u32>().min_value(1).max_value(3));
            let keep = tc.draw(gs::booleans());
            self.armed_op(
                &tc,
                "retain_modulo",
                // `retain_modulo` loops over per-step atomic operations.
                PanicSafety::StepAtomic,
                |map| {
                    map.retain(|item| {
                        let matches = item.key1 % modulo == rem;
                        if keep { matches } else { !matches }
                    });
                },
            );
        }

        #[rule]
        fn extend(&mut self, tc: TestCase) {
            let triples = tc.draw(
                gs::vecs(gs::tuples!(
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                ))
                .max_size(7),
            );
            // `extend` does per-step atomic operations.
            self.armed_op(&tc, "extend", PanicSafety::StepAtomic, |map| {
                map.extend(triples.into_iter().map(|(key1, key2, key3)| {
                    PanickyHashItem { key1, key2, key3 }
                }));
            });
        }

        #[rule]
        fn fill(&mut self, tc: TestCase) {
            let triples = tc.draw(
                gs::vecs(gs::tuples!(
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                ))
                .max_size(64),
            );
            for (key1, key2, key3) in triples {
                let item = PanickyHashItem { key1, key2, key3 };
                let _ = self.map.insert_unique(item);
            }
        }

        #[rule]
        fn clear(&mut self, tc: TestCase) {
            self.armed_op(
                &tc,
                "clear",
                // `clear` does per-table atomic operations.
                PanicSafety::StepAtomic,
                |map| {
                    map.clear();
                },
            );
        }

        #[rule]
        fn shrink_to_fit(&mut self, tc: TestCase) {
            self.armed_op(&tc, "shrink_to_fit", PanicSafety::Atomic, |map| {
                map.shrink_to_fit();
            });
        }

        #[rule]
        fn shrink_to(&mut self, tc: TestCase) {
            let min_capacity = tc.draw(
                gs::integers::<usize>().max_value(MAX_PANIC_KEY as usize),
            );
            self.armed_op(&tc, "shrink_to", PanicSafety::Atomic, |map| {
                map.shrink_to(min_capacity);
            });
        }

        #[invariant]
        fn check_post_op(&mut self, _: TestCase) {
            let Some(p) = self.pending.take() else {
                self.map
                    .validate(ValidateCompact::NonCompact)
                    .expect("map should be valid");
                return;
            };
            let step = self.step;

            // `NonCompact` since step-atomic panics can leave compactness in an
            // indeterminate state.
            self.map.validate(ValidateCompact::NonCompact).unwrap_or_else(
                |err| {
                    panic!(
                        "map invalid after op {step} ({}, armed: {:?}, \
                         panicked: {}): {err}",
                        p.label, p.armed, p.panicked
                    )
                },
            );
            let post_state = sorted_keys(&self.map, |item| {
                (item.key1, item.key2, item.key3)
            });
            assert_post_op_invariants(
                step,
                &p.label,
                p.armed,
                p.panicked,
                p.panic_safety,
                &p.pre_state,
                &post_state,
                |&(k1, k2, k3)| {
                    self.map.contains_key1(&PanickySearchKey(k1))
                        && self.map.contains_key2(&PanickySearchKey(k2))
                        && self.map.contains_key3(&PanickySearchKey(k3))
                },
            );
            self.step += 1;
        }
    }

    #[hegel::test(test_cases = 512)]
    fn proptest_panic_ops(tc: TestCase) {
        let map = PanickyMap::with_hasher_in(
            iddqd::DefaultHashBuilder::default(),
            PanickyAlloc::default(),
        );
        hegel::stateful::run(PanicMachine { map, step: 0, pending: None }, tc);
    }
}
