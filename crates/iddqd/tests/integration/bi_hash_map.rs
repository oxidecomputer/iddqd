use crate::hegel_support::{
    draw_fill_batch, draw_lookup_key1, draw_lookup_key2, draw_lookup_keys12,
    draw_shuffle, test_item,
};
use hegel::{TestCase, generators as gs};
use iddqd::{
    BiHashItem, BiHashMap, bi_hash_map, bi_upcast, internal::ValidateCompact,
};
use iddqd_test_utils::{
    borrowed_item::BorrowedItem,
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_item::{
        Alloc, HashBuilder, ItemMap, TestItem, TestKey1, TestKey2,
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
}

impl BiHashItem for SimpleItem {
    type K1<'a> = u32;
    type K2<'a> = char;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    bi_upcast!();
}

#[test]
fn debug_impls() {
    let mut map = BiHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(SimpleItem { key1: 1, key2: 'a' }).unwrap();
    map.insert_unique(SimpleItem { key1: 20, key2: 'b' }).unwrap();
    map.insert_unique(SimpleItem { key1: 10, key2: 'c' }).unwrap();

    assert_eq!(
        format!("{map:?}"),
        // Iteration is in insertion order.
        "{{k1: 1, k2: 'a'}: SimpleItem { key1: 1, key2: 'a' }, \
          {k1: 20, k2: 'b'}: SimpleItem { key1: 20, key2: 'b' }, \
          {k1: 10, k2: 'c'}: SimpleItem { key1: 10, key2: 'c' }}",
    );
    assert_eq!(
        format!("{:?}", map.get1_mut(&1).unwrap()),
        "SimpleItem { key1: 1, key2: 'a' }"
    );
}

#[test]
fn debug_impls_borrowed() {
    let before = bi_hash_map! {
        HashBuilder;
        BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
        BorrowedItem { key1: "b", key2: Cow::Borrowed(b"b1"), key3: Path::new("path1") },
        BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b2"), key3: Path::new("path2") },
    };

    assert_eq!(
        format!("{before:?}"),
        r#"{{k1: "a", k2: [98, 48]}: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, {k1: "b", k2: [98, 49]}: BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }, {k1: "c", k2: [98, 50]}: BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }}"#
    );

    #[cfg(feature = "daft")]
    {
        use daft::Diffable;

        let after = bi_hash_map! {
            HashBuilder;
            BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
            BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b3"), key3: Path::new("path3") },
            BorrowedItem { key1: "d", key2: Cow::Borrowed(b"b4"), key3: Path::new("path4") },
        };

        let diff = before.diff(&after).by_unique();
        assert_eq!(
            format!("{diff:?}"),
            r#"Diff { common: {{k1: "a", k2: [98, 48]}: IdLeaf { before: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, after: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" } }}, added: {{k1: "c", k2: [98, 51]}: BorrowedItem { key1: "c", key2: [98, 51], key3: "path3" }, {k1: "d", k2: [98, 52]}: BorrowedItem { key1: "d", key2: [98, 52], key3: "path4" }}, removed: {{k1: "b", k2: [98, 49]}: BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }, {k1: "c", k2: [98, 50]}: BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }} }"#
        );
    }
}

#[test]
fn with_capacity() {
    let map = BiHashMap::<TestItem, HashBuilder>::with_capacity_and_hasher(
        1024,
        HashBuilder::default(),
    );
    assert!(map.capacity() >= 1024);
}

#[test]
fn test_insert_unique() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add an element.
    let v1 = TestItem::new(0, 'a', "x", "v");
    map.insert_unique(v1.clone()).unwrap();

    // Add an exact duplicate, which should error out.
    let error = map.insert_unique(v1.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v1);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key1, which should error out.
    let v2 = TestItem::new(0, 'b', "x", "v");
    let error = map.insert_unique(v2.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v2);
    assert_eq!(error.duplicates(), vec![&v1]);

    // Add a duplicate against just key2, which should error out.
    let v3 = TestItem::new(1, 'a', "x", "v");
    let error = map.insert_unique(v3.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v3);

    // Add an item that doesn't have any conflicts. (key3 is the same, but
    // BiHashMap doesn't index on it.)
    let v4 = TestItem::new(1, 'b', "x", "v");
    map.insert_unique(v4.clone()).unwrap();

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    {
        let mut items: Vec<bi_hash_map::RefMut<_, HashBuilder>> =
            map.iter_mut().collect();
        items.sort_by(|a, b| a.key1().cmp(&b.key1()));
        let e1 = &items[0];
        assert_eq!(**e1, v1);

        // Test that the RefMut Debug impl looks good.
        assert!(
            format!("{e1:?}").starts_with(
                r#"TestItem { key1: 0, key2: 'a', key3: "x", value: "v""#,
            ),
            "RefMut Debug impl should forward to TestItem"
        );

        let e2 = &*items[1];
        assert_eq!(*e2, v4);
    }

    // Check that the *unique methods work.
    assert!(map.contains_key_unique(&v4.key1(), &v4.key2()));
    assert_eq!(map.get_unique(&v4.key1(), &v4.key2()), Some(&v4));
    assert_eq!(*map.get_mut_unique(&v4.key1(), &v4.key2()).unwrap(), &v4);
    assert_eq!(map.remove_unique(&v4.key1(), &v4.key2()), Some(v4));
}

// Test that the unsafe block within RefMut doesn't trip up miri.
#[test]
fn test_ref_mut_aliasing() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    for i in 0..16_u8 {
        let key2 = (b'a' + i) as char;
        map.insert_unique(TestItem::new(i, key2, "x", "v")).unwrap();
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

#[test]
fn test_extend() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let items = vec![
        TestItem::new(1, 'a', "x", "v"),
        TestItem::new(2, 'b', "y", "w"),
        TestItem::new(3, 'c', "a", "b"),
        TestItem::new(1, 'c', "z", "overwrote key1"),
        TestItem::new(3, 'b', "q", "overwrote key1 and key2"),
        TestItem::new(4, 'x', "y", "z"),
    ];
    map.extend(items.clone());
    assert_eq!(map.len(), 3);
    assert_eq!(map.get1(&TestKey1::new(&1)).unwrap().value, "overwrote key1");
    assert_eq!(map.get1(&TestKey1::new(&2)), None);
    assert_eq!(
        map.get1(&TestKey1::new(&3)).unwrap().value,
        "overwrote key1 and key2"
    );
    assert_eq!(map.get1(&TestKey1::new(&4)).unwrap().value, "z");
}

// Example-based test for insert_overwrite.
//
// Can be used to write down examples seen from the property-based operation
// test, for easier debugging.
#[test]
fn test_insert_overwrite() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();

    // Add an element.
    let v1 = TestItem::new(20, 'a', "x", "v");
    assert_eq!(map.insert_overwrite(v1.clone()), Vec::<TestItem>::new());

    // Add an element with the same keys but a different value.
    let v2 = TestItem::new(20, 'a', "y", "w");
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

struct BiHashMapMachine {
    map: BiHashMap<TestItem, HashBuilder, Alloc>,
    naive: NaiveMap,
    compactness: ValidateCompact,
}

impl BiHashMapMachine {
    fn check_valid(&mut self, change: CompactnessChange) {
        self.compactness = change.apply(self.compactness);
        self.map.validate(self.compactness).expect("map should be valid");
    }
}

mod indent0 {
    mod indent1 {
        use super::super::*;

        #[hegel::state_machine]
        impl BiHashMapMachine {
            #[rule]
            fn insert_unique(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let item = tc.draw(test_item());
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
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let item = tc.draw(test_item());
                let mut map_dups = map.insert_overwrite(item.clone());
                map_dups.sort();
                let mut naive_dups = naive_map.insert_overwrite(item.clone());
                naive_dups.sort();

                assert_eq!(
                    map_dups, naive_dups,
                    "map and naive map should agree on insert_overwrite dups"
                );
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn entry_insert_overwrite(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let item = tc.draw(test_item());
                let map_res = match map.entry(item.key1(), item.key2()) {
                    bi_hash_map::Entry::Occupied(mut entry) => {
                        let mut dups = entry.insert(item.clone());
                        dups.sort();
                        Some(dups)
                    }
                    bi_hash_map::Entry::Vacant(_) => None,
                };

                let occupied = naive_map.get1(item.key1).is_some()
                    || naive_map.get2(item.key2).is_some();
                let naive_res = occupied.then(|| {
                    let mut dups = naive_map.insert_overwrite(item.clone());
                    dups.sort();
                    dups
                });

                assert_eq!(
                    map_res, naive_res,
                    "map and naive map should agree on Entry::insert dups"
                );
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn entry_remove(&mut self, tc: TestCase) {
                let (key1, key2) = draw_lookup_keys12(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;

                let map_res = match map
                    .entry(TestKey1::new(&key1), TestKey2::new(key2))
                {
                    bi_hash_map::Entry::Occupied(entry) => {
                        let mut removed = entry.remove();
                        removed.sort();
                        removed
                    }
                    bi_hash_map::Entry::Vacant(_) => Vec::new(),
                };

                let mut naive_res = naive_map.entry_remove12(key1, key2);
                naive_res.sort();

                assert_eq!(
                    map_res, naive_res,
                    "map and naive map should agree on Entry::remove items"
                );
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn get1(&mut self, tc: TestCase) {
                let key1 = draw_lookup_key1(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map.get1(&TestKey1::new(&key1));
                let naive_res = naive_map.get1(key1);

                assert_eq!(map_res, naive_res);
            }

            #[rule]
            fn get2(&mut self, tc: TestCase) {
                let key2 = draw_lookup_key2(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map.get2(&TestKey2::new(key2));
                let naive_res = naive_map.get2(key2);

                assert_eq!(map_res, naive_res);
            }

            #[rule]
            fn get_unique(&mut self, tc: TestCase) {
                let (key1, key2) = draw_lookup_keys12(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res =
                    map.get_unique(&TestKey1::new(&key1), &TestKey2::new(key2));
                let naive_res = naive_map.get_unique12(key1, key2);

                assert_eq!(map_res, naive_res);
            }

            #[rule]
            fn get_mut_unique(&mut self, tc: TestCase) {
                let (key1, key2) = draw_lookup_keys12(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map
                    .get_mut_unique(&TestKey1::new(&key1), &TestKey2::new(key2))
                    .map(|r| (*r).clone());
                let naive_res = naive_map.get_mut_unique12(key1, key2).cloned();

                assert_eq!(map_res, naive_res);
                self.check_valid(CompactnessChange::NoChange);
            }

            #[rule]
            fn remove1(&mut self, tc: TestCase) {
                let key1 = draw_lookup_key1(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map.remove1(&TestKey1::new(&key1));
                let naive_res = naive_map.remove1(key1);

                assert_eq!(map_res, naive_res);
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn remove2(&mut self, tc: TestCase) {
                let key2 = draw_lookup_key2(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map.remove2(&TestKey2::new(key2));
                let naive_res = naive_map.remove2(key2);

                assert_eq!(map_res, naive_res);
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn remove_unique(&mut self, tc: TestCase) {
                let (key1, key2) = draw_lookup_keys12(&tc, &self.naive);
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let map_res = map
                    .remove_unique(&TestKey1::new(&key1), &TestKey2::new(key2));
                let naive_res = naive_map.remove_unique12(key1, key2);

                assert_eq!(map_res, naive_res);
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn retain_value_contains(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let ch = tc.draw(gs::characters());
                let equals = tc.draw(gs::booleans());
                map.retain(|item| {
                    let contains = item.value.contains(ch);
                    if equals { contains } else { !contains }
                });
                naive_map.retain(|item| {
                    let contains = item.value.contains(ch);
                    if equals { contains } else { !contains }
                });
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn retain_modulo(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let a = tc.draw(gs::integers::<u8>().max_value(2));
                let b = tc.draw(gs::integers::<u8>().min_value(1).max_value(3));
                let equals = tc.draw(gs::booleans());
                let modulo = a + b;
                let remainder = a;
                map.retain(|item| {
                    let matches = item.key1 % modulo == remainder;
                    if equals { matches } else { !matches }
                });
                naive_map.retain(|item| {
                    let matches = item.key1 % modulo == remainder;
                    if equals { matches } else { !matches }
                });
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn extend(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let items = tc.draw(gs::vecs(test_item()).max_size(15));
                map.extend(items.clone());
                naive_map.extend(items);
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            // Fill up the map to ensure later operations use a larger map.
            #[rule]
            fn fill(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                let items = draw_fill_batch(&tc);
                map.extend(items.clone());
                naive_map.extend(items);
                self.check_valid(CompactnessChange::NoLongerCompact);
            }

            #[rule]
            fn reserve(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let additional =
                    tc.draw(gs::integers::<usize>().max_value(255));
                map.reserve(additional);
                // `reserve` has no observable effect beyond capacity -- the
                // naive map has no equivalent. `check_valid` will iterate items
                // and ask `find_index` for each, which catches a hash-table
                // left mis-bucketed by a regrowth rehash.
                self.check_valid(CompactnessChange::NoChange);
            }

            #[rule]
            fn try_reserve(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let additional =
                    tc.draw(gs::integers::<usize>().max_value(255));
                let _ = map.try_reserve(additional);
                // See the comment on `reserve` above for why this is only
                // `check_valid`.
                self.check_valid(CompactnessChange::NoChange);
            }

            #[rule]
            fn shrink_to_fit(&mut self, _: TestCase) {
                let map = &mut self.map;
                map.shrink_to_fit();
                self.check_valid(CompactnessChange::BecomesCompact);
            }

            #[rule]
            fn shrink_to(&mut self, tc: TestCase) {
                let map = &mut self.map;
                let min_capacity =
                    tc.draw(gs::integers::<usize>().max_value(255));
                map.shrink_to(min_capacity);
                self.check_valid(CompactnessChange::BecomesCompact);
            }

            #[rule]
            fn clear(&mut self, _: TestCase) {
                let map = &mut self.map;
                let naive_map = &mut self.naive;
                map.clear();
                naive_map.clear();
                self.check_valid(CompactnessChange::BecomesCompact);
            }

            #[invariant]
            fn iter_matches(&mut self, _: TestCase) {
                let map = &self.map;
                let naive_map = &self.naive;
                let mut naive_items = naive_map.iter().collect::<Vec<_>>();
                naive_items.sort_by(|a, b| a.key1().cmp(&b.key1()));
                assert_iter_eq(map.clone(), naive_items);
            }
        }
    }
}

#[hegel::test(test_cases = 512)]
fn proptest_ops(tc: TestCase) {
    let machine = BiHashMapMachine {
        map: BiHashMap::<TestItem, HashBuilder, Alloc>::make_new(),
        naive: NaiveMap::new_key12(),
        compactness: ValidateCompact::Compact,
    };
    hegel::stateful::run(machine, tc);
}

#[hegel::test(test_cases = 64)]
fn proptest_permutation_eq(tc: TestCase) {
    // draw_fill_batch generates unique keys so there's no need to deduplicate.
    let set = draw_fill_batch(&tc);
    let set2 = draw_shuffle(&tc, &set);

    let mut map1 = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
    let mut map1 = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let mut map2 = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();

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
        // Insert an item with a different key1.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(2, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);
    }

    {
        // Insert an item with a different key2.
        let mut map1 = map1.clone();
        map1.insert_unique(TestItem::new(1, 'b', "y", "v")).unwrap();
        assert_ne_props(&map1, &map2);

        let mut map2 = map2.clone();
        map2.insert_unique(TestItem::new(1, 'c', "y", "v")).unwrap();
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
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get1_mut(&TestKey1::new(&128)).unwrap().key1 = 2;
}

#[test]
#[should_panic(expected = "key2 changed during RefMut borrow")]
fn get_mut_panics_if_key2_changes() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get1_mut(&TestKey1::new(&128)).unwrap().key2 = 'c';
}

#[test]
fn entry_examples() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    let item1 = TestItem::new(0, 'a', "x", "v");

    let bi_hash_map::Entry::Vacant(entry) =
        map.entry(item1.key1(), item1.key2())
    else {
        panic!("expected VacantEntry")
    };
    let mut entry = entry.insert_entry(item1.clone());

    assert!(entry.is_unique());
    assert!(!entry.is_non_unique());
    assert_eq!(entry.get().as_unique(), Some(&item1));
    assert_eq!(entry.get().by_key1(), Some(&item1));
    assert_eq!(entry.get().by_key2(), Some(&item1));
    assert_eq!(entry.get_mut().as_unique().unwrap().into_ref(), &item1);
    assert_eq!(entry.get_mut().by_key1().unwrap().into_ref(), &item1);
    assert_eq!(entry.get_mut().by_key2().unwrap().into_ref(), &item1);
    assert_eq!(entry.into_ref().as_unique(), Some(&item1));

    // Test a non-unique item.
    let item2 = TestItem::new(0, 'b', "x", "v");
    let bi_hash_map::Entry::Occupied(mut entry) =
        map.entry(item2.key1(), item2.key2())
    else {
        panic!("expected OccupiedEntry")
    };

    assert!(!entry.is_unique());
    assert!(entry.is_non_unique());
    assert_eq!(entry.get().as_unique(), None);
    assert_eq!(entry.get().by_key1(), Some(&item1));
    assert_eq!(entry.get().by_key2(), None);
    assert!(entry.get_mut().as_unique().is_none());
    assert_eq!(entry.get_mut().by_key1().unwrap().into_ref(), &item1);
    assert!(entry.get_mut().by_key2().is_none());

    // Try inserting item2 into the map.
    let old_items = entry.insert(item2.clone());
    assert_eq!(old_items, vec![item1]);

    // The entry should now be unique.
    assert!(entry.is_unique());
    assert!(!entry.is_non_unique());
    assert_eq!(entry.get().as_unique(), Some(&item2));
    assert_eq!(entry.get().by_key1(), Some(&item2));
    assert_eq!(entry.get().by_key2(), Some(&item2));

    // Try removing the entry.
    let removed = entry.remove();
    assert_eq!(removed, vec![item2]);
    // There should be no items left in the map.
    assert_eq!(map.len(), 0);

    // Try adding an item with or_insert_with.
    let item3 = TestItem::new(1, 'c', "x", "v");
    {
        let mut item3_mut = map
            .entry(item3.key1(), item3.key2())
            .or_insert_with(|| item3.clone());
        assert_eq!(item3_mut.as_unique().unwrap().into_ref(), &item3);
    }

    // item4 has some conflicts so it should *not* be inserted via the
    // or_insert_with path.
    let item4 = TestItem::new(1, 'd', "x", "v");
    {
        let mut item3_mut = map
            .entry(item4.key1(), item4.key2())
            .or_insert_with(|| item4.clone());
        assert_eq!(item3_mut.by_key1().unwrap().into_ref(), &item3);
    }

    // item5 has no conflicts.
    let item5 = TestItem::new(2, 'e', "x", "v");
    {
        let mut item5_mut = map
            .entry(item5.key1(), item5.key2())
            .or_insert_with(|| item5.clone());
        assert_eq!(item5_mut.as_unique().unwrap().into_ref(), &item5);
    }

    // item6 conflicts with both item3 and item5, so `and_modify` should be
    // called twice.
    let item6 = TestItem::new(2, 'c', "x", "v");
    {
        let mut item3_seen = false;
        let mut item5_seen = false;
        let entry = map.entry(item6.key1(), item6.key2()).and_modify(|item| {
            if *item == item3 {
                item3_seen = true;
            } else if *item == item5 {
                item5_seen = true;
            }
        });
        assert!(matches!(entry, bi_hash_map::Entry::Occupied(_)));
        assert!(item3_seen);
        assert!(item5_seen);
    }
}

// A NonUnique occupied entry resolves `by_key1` and `by_key2` to *different*
// underlying items; `into_mut` must hand back disjoint `&mut`s to both.
#[test]
fn entry_nonunique_writes_through_both_keys() {
    #[derive(Clone, Debug)]
    struct BiItem {
        k1: u32,
        k2: u32,
        payload: u64,
    }

    impl BiHashItem for BiItem {
        type K1<'a> = u32;
        type K2<'a> = u32;
        fn key1(&self) -> Self::K1<'_> {
            self.k1
        }
        fn key2(&self) -> Self::K2<'_> {
            self.k2
        }
        bi_upcast!();
    }

    let mut map = BiHashMap::<BiItem, HashBuilder, Alloc>::make_new();
    for i in 0..8u32 {
        map.insert_unique(BiItem { k1: i, k2: 1000 + i, payload: 0 }).unwrap();
    }

    // k1 from item 3, k2 from item 5 -> NonUnique -> Key12 -> get_disjoint_mut.
    let occupied = match map.entry(3, 1005) {
        bi_hash_map::Entry::Occupied(o) => o,
        bi_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
    };
    assert!(occupied.is_non_unique());
    let mut entry_mut = occupied.into_mut();
    if let Some(mut r) = entry_mut.by_key1() {
        r.payload = 1111;
    }
    if let Some(mut r) = entry_mut.by_key2() {
        r.payload = 2222;
    }
    drop(entry_mut);

    assert_eq!(map.get1(&3).unwrap().payload, 1111);
    assert_eq!(map.get1(&5).unwrap().payload, 2222);
}

#[test]
#[should_panic = "key1 hashes do not match"]
fn insert_panics_for_non_matching_key1() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'b', "bar", "value");
    let entry = map.entry(TestKey1::new(&2), TestKey2::new('b'));
    assert!(matches!(entry, bi_hash_map::Entry::Vacant(_)));
    // Try inserting v2 which matches v1's key2 but not key1.
    entry.or_insert(v2);
}

#[test]
#[should_panic = "key2 hashes do not match"]
fn insert_panics_for_non_matching_key2() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'b', "bar", "value");
    let entry = map.entry(TestKey1::new(&1), TestKey2::new('c'));
    assert!(matches!(entry, bi_hash_map::Entry::Vacant(_)));
    // Try inserting v2 which matches v1's key1 but not key2.
    entry.or_insert(v2);
}

#[test]
fn entry_insert_non_matching_key1() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key1(), v2.key2());
    let bi_hash_map::Entry::Occupied(mut entry) = entry else {
        panic!("expected OccupiedEntry");
    };
    // Try inserting v1, which is present in the map.
    assert!(!entry.is_unique(), "only key1 matches");
    let old_items = entry.insert(v2);
    assert_eq!(old_items, vec![v1]);
    assert!(entry.is_unique(), "entry is now unique");
}

#[test]
fn entry_insert_non_matching_key2() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(0, 'b', "bar", "value");
    let entry = map.entry(v2.key1(), v2.key2());
    let bi_hash_map::Entry::Occupied(mut entry) = entry else {
        panic!("expected OccupiedEntry");
    };
    // Try inserting v1, which is present in the map.
    assert!(!entry.is_unique(), "only key1 matches");
    let old_items = entry.insert(v2);
    assert_eq!(old_items, vec![v1]);
    assert!(entry.is_unique(), "entry is now unique");
}

#[test]
#[should_panic = "key1 hashes do not match"]
fn insert_entry_panics_for_non_matching_keys() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = BiHashMap::<_, HashBuilder, Alloc>::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'b', "bar", "value");
    let entry = map.entry(v2.key1(), v2.key2());
    assert!(matches!(entry, bi_hash_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    if let bi_hash_map::Entry::Vacant(entry) = entry {
        entry.insert_entry(v1);
    } else {
        panic!("expected VacantEntry");
    }
}

#[test]
fn borrowed_item() {
    let mut map = BiHashMap::<BorrowedItem, HashBuilder, Alloc>::default();
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
        map: &BiHashMap<BorrowedItem<'_>, HashBuilder, Alloc>,
    ) -> String {
        format!("{map:?}")
    }

    #[cfg(feature = "serde")]
    fn serialize_as_map(
        map: &BiHashMap<BorrowedItem<'_>, HashBuilder, Alloc>,
    ) -> Result<String, iddqd_test_utils::serde_json::Error> {
        let mut out: Vec<u8> = Vec::new();
        let mut ser = iddqd_test_utils::serde_json::Serializer::new(&mut out);
        bi_hash_map::BiHashMapAsMap::serialize(map, &mut ser)?;
        Ok(String::from_utf8(out)
            .expect("serde_json should always emit valid UTF-8"))
    }

    static DEBUG_OUTPUT: &str = "{{k1: \"foo\", k2: [102, 111, 111]}: BorrowedItem { \
        key1: \"foo\", key2: [102, 111, 111], key3: \"foo\" }, \
        {k1: \"bar\", k2: [98, 97, 114]}: BorrowedItem { \
        key1: \"bar\", key2: [98, 97, 114], key3: \"bar\" }}";

    assert_eq!(format!("{map:?}"), DEBUG_OUTPUT);
    assert_eq!(fmt_debug(&map), DEBUG_OUTPUT);

    #[cfg(feature = "serde")]
    {
        let map_string = serialize_as_map(&map).unwrap();
        let deserialized: BiHashMap<BorrowedItem<'_>, HashBuilder, Alloc> =
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

    let mut map = BiHashMap::<BorrowedItem<'_>, HashBuilder, Alloc>::default();
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
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    map.retain(|_| false);

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn test_retain_value_contains() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
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
    let mut large_map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    for i in 0..32_u8 {
        large_map
            .insert_unique(TestItem::new(i, char::from(b'a' + i), "y", "z"))
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
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.retain(|_| true);
    assert!(map.is_empty());
}

#[test]
fn test_clear_empty_map() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.clear();
    assert!(map.is_empty());
    map.validate(ValidateCompact::Compact)
        .expect("empty cleared map should be compact");
}

#[test]
fn test_clear_makes_compact() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();

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
fn test_retain_verifies_both_keys() {
    let mut map = BiHashMap::<TestItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    // Retain only key1=2
    map.retain(|item| item.key1 == 2);

    // Verify both key1 and key2 work
    map.get1(&TestKey1::new(&2)).expect("key1=2 should be present");
    map.get2(&TestKey2::new('b')).expect("key2='b' should be present");
    assert!(map.get1(&TestKey1::new(&1)).is_none());
    assert!(map.get2(&TestKey2::new('a')).is_none());
}

mod macro_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct User {
        id: u32,
        name: String,
    }

    impl BiHashItem for User {
        type K1<'a> = u32;
        type K2<'a> = &'a str;
        fn key1(&self) -> Self::K1<'_> {
            self.id
        }
        fn key2(&self) -> Self::K2<'_> {
            &self.name
        }
        bi_upcast!();
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_basic() {
        let map = bi_hash_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 2, name: "Bob".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get1(&1).unwrap().name, "Alice");
        assert_eq!(map.get2("Bob").unwrap().id, 2);
    }

    #[test]
    fn macro_with_hasher() {
        let map = bi_hash_map! {
            HashBuilder;
            User { id: 3, name: "Charlie".to_string() },
            User { id: 4, name: "David".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get1(&3).unwrap().name, "Charlie");
        assert_eq!(map.get2("David").unwrap().id, 4);
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_empty() {
        let empty_map: BiHashMap<User> = bi_hash_map! {};
        assert!(empty_map.is_empty());
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    fn macro_without_trailing_comma() {
        let map = bi_hash_map! {
            User { id: 1, name: "Alice".to_string() }
        };
        assert_eq!(map.len(), 1);
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key1() {
        let _map = bi_hash_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 1, name: "Bob".to_string() },
        };
    }

    #[cfg(feature = "default-hasher")]
    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key2() {
        let _map = bi_hash_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 2, name: "Alice".to_string() },
        };
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use crate::hegel_support::draw_random_batch;
    use hegel::TestCase;
    use iddqd::BiHashMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip,
        test_item::{Alloc, HashBuilder, TestItem},
    };

    #[hegel::test(test_cases = 256)]
    fn proptest_serialize_roundtrip(tc: TestCase) {
        let values = draw_random_batch(&tc);
        assert_serialize_roundtrip::<BiHashMap<TestItem, HashBuilder, Alloc>>(
            values,
        );
    }
}

#[cfg(feature = "proptest")]
use test_strategy::proptest;

#[cfg(feature = "proptest")]
#[proptest(cases = 16)]
fn proptest_arbitrary_map(map: BiHashMap<TestItem, HashBuilder, Alloc>) {
    // Test that the arbitrarily generated map is valid.
    map.validate(ValidateCompact::NonCompact).expect("map should be valid");

    // Test that we can perform basic operations on the generated map.
    let len = map.len();
    assert_eq!(map.is_empty(), len == 0);

    // Test that we can iterate over the map.
    let mut count = 0;
    for item in &map {
        count += 1;
        // Each item should be findable by both keys.
        assert_eq!(map.get1(&item.key1()), Some(item));
        assert_eq!(map.get2(&item.key2()), Some(item));
    }
    assert_eq!(count, len);
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
#[derive(Clone, Debug)]
struct PanickyHashItem {
    key1: u32,
    key2: u32,
}

#[cfg(all(feature = "default-hasher", feature = "allocator-api2"))]
impl BiHashItem for PanickyHashItem {
    type K1<'a> = iddqd_test_utils::panic_safety::PanickyKey;
    type K2<'a> = iddqd_test_utils::panic_safety::PanickyKey;

    fn key1(&self) -> Self::K1<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key1");
        iddqd_test_utils::panic_safety::PanickyKey(self.key1)
    }

    fn key2(&self) -> Self::K2<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key2");
        iddqd_test_utils::panic_safety::PanickyKey(self.key2)
    }

    bi_upcast!();
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
        PanicSafety, PanickyAlloc, PanickyKey, PanickySearchKey,
        assert_panic_fired_as_expected, assert_post_op_invariants,
        drop_unarmed, record_observation, run_armed, sorted_keys,
    };

    type PanickyMap = BiHashMap<
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
        pre_state: Vec<(u32, u32)>,
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
            let pre_state =
                sorted_keys(&self.map, |item| (item.key1, item.key2));
            let (panicked, ops) = run_armed(armed, || op(&mut self.map));
            record_observation("bi_hash_map", label, ops);
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
            self.armed_op(&tc, "insert_unique", PanicSafety::Atomic, |map| {
                drop_unarmed(map.insert_unique(PanickyHashItem { key1, key2 }));
            });
        }

        #[rule]
        fn insert_overwrite(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(
                &tc,
                "insert_overwrite",
                PanicSafety::Atomic,
                |map| {
                    drop_unarmed(
                        map.insert_overwrite(PanickyHashItem { key1, key2 }),
                    );
                },
            );
        }

        #[rule]
        fn entry_insert_overwrite(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(
                &tc,
                "entry_insert_overwrite",
                PanicSafety::Atomic,
                |map| {
                    let entry = map.entry(PanickyKey(key1), PanickyKey(key2));
                    if let bi_hash_map::Entry::Occupied(mut entry) = entry {
                        drop_unarmed(
                            entry.insert(PanickyHashItem { key1, key2 }),
                        );
                    }
                },
            );
        }

        #[rule]
        fn entry_remove(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "entry_remove", PanicSafety::Atomic, |map| {
                let entry = map.entry(PanickyKey(key1), PanickyKey(key2));
                if let bi_hash_map::Entry::Occupied(entry) = entry {
                    drop_unarmed(entry.remove());
                }
            });
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
        fn contains_key1(&mut self, tc: TestCase) {
            let key1 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "contains_key1", PanicSafety::Atomic, |map| {
                let _ = map.contains_key1(&PanickySearchKey(key1));
            });
        }

        #[rule]
        fn contains_key2(&mut self, tc: TestCase) {
            let key2 = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "contains_key2", PanicSafety::Atomic, |map| {
                let _ = map.contains_key2(&PanickySearchKey(key2));
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
            let pairs = tc.draw(
                gs::vecs(gs::tuples!(
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                ))
                .max_size(7),
            );
            // `extend` does per-step atomic operations.
            self.armed_op(&tc, "extend", PanicSafety::StepAtomic, |map| {
                map.extend(
                    pairs
                        .into_iter()
                        .map(|(key1, key2)| PanickyHashItem { key1, key2 }),
                );
            });
        }

        #[rule]
        fn fill(&mut self, tc: TestCase) {
            let pairs = tc.draw(
                gs::vecs(gs::tuples!(
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                    gs::integers::<u32>().max_value(MAX_PANIC_KEY),
                ))
                .max_size(64),
            );
            for (key1, key2) in pairs {
                let _ = self.map.insert_unique(PanickyHashItem { key1, key2 });
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
            let post_state =
                sorted_keys(&self.map, |item| (item.key1, item.key2));
            assert_post_op_invariants(
                step,
                &p.label,
                p.armed,
                p.panicked,
                p.panic_safety,
                &p.pre_state,
                &post_state,
                |&(k1, k2)| {
                    self.map.contains_key1(&PanickySearchKey(k1))
                        && self.map.contains_key2(&PanickySearchKey(k2))
                },
            );
            self.step += 1;
        }
    }

    #[hegel::test(test_cases = 512)]
    fn proptest_panic_ops(tc: TestCase) {
        let map: PanickyMap = BiHashMap::with_hasher_in(
            iddqd::DefaultHashBuilder::default(),
            PanickyAlloc::default(),
        );
        hegel::stateful::run(PanicMachine { map, step: 0, pending: None }, tc);
    }
}
