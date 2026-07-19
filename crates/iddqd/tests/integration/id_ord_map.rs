use crate::hegel_support::{
    draw_fill_batch, draw_lookup_key1, draw_shuffle, test_item,
};
use hegel::{TestCase, generators as gs};
use iddqd::{
    IdOrdItem, IdOrdMap, id_ord_map, id_upcast,
    internal::{ValidateChaos, ValidateCompact},
};
use iddqd_test_utils::{
    borrowed_item::BorrowedItem,
    eq_props::{assert_eq_props, assert_ne_props},
    naive_map::NaiveMap,
    test_item::{
        ChaosEq, ChaosOrd, ItemMap, KeyChaos, TestItem, TestKey1,
        assert_iter_eq, without_chaos,
    },
    unwind::catch_panic,
};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

#[test]
fn with_capacity() {
    let map = IdOrdMap::<TestItem>::with_capacity(1024);
    assert!(map.capacity() >= 1024);
}

#[test]
fn test_extend() {
    let mut map = IdOrdMap::<TestItem>::make_new();
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

#[derive(Clone, Debug)]
struct SimpleItem {
    key: u32,
}

impl IdOrdItem for SimpleItem {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.key
    }

    id_upcast!();
}

#[test]
fn debug_impls() {
    let mut map = IdOrdMap::<SimpleItem>::make_new();
    map.insert_unique(SimpleItem { key: 1 }).unwrap();
    map.insert_unique(SimpleItem { key: 20 }).unwrap();
    map.insert_unique(SimpleItem { key: 10 }).unwrap();

    assert_eq!(
        format!("{map:?}"),
        r#"{1: SimpleItem { key: 1 }, 10: SimpleItem { key: 10 }, 20: SimpleItem { key: 20 }}"#
    );
    assert_eq!(
        format!("{:?}", map.get_mut(&1).unwrap()),
        "SimpleItem { key: 1 }"
    );
}

// Ensure that Debug impls work for borrowed items, including diff
// implementations.
#[test]
fn debug_impls_borrowed() {
    let before = id_ord_map! {
        BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
        BorrowedItem { key1: "b", key2: Cow::Borrowed(b"b1"), key3: Path::new("path1") },
        BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b2"), key3: Path::new("path2") },
    };

    assert_eq!(
        format!("{before:?}"),
        r#"{"a": BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, "b": BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }, "c": BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }}"#
    );

    #[cfg(feature = "daft")]
    {
        use daft::Diffable;

        let after = id_ord_map! {
            BorrowedItem { key1: "a", key2: Cow::Borrowed(b"b0"), key3: Path::new("path0") },
            BorrowedItem { key1: "c", key2: Cow::Borrowed(b"b3"), key3: Path::new("path3") },
            BorrowedItem { key1: "d", key2: Cow::Borrowed(b"b4"), key3: Path::new("path4") },
        };

        let diff = before.diff(&after);
        assert_eq!(
            format!("{diff:?}"),
            r#"Diff { common: {"a": IdLeaf { before: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" }, after: BorrowedItem { key1: "a", key2: [98, 48], key3: "path0" } }, "c": IdLeaf { before: BorrowedItem { key1: "c", key2: [98, 50], key3: "path2" }, after: BorrowedItem { key1: "c", key2: [98, 51], key3: "path3" } }}, added: {"d": BorrowedItem { key1: "d", key2: [98, 52], key3: "path4" }}, removed: {"b": BorrowedItem { key1: "b", key2: [98, 49], key3: "path1" }} }"#
        );
    }
}

#[test]
fn test_compact_chaos() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    let mut chaos_eq = ChaosEq::all_variants().into_iter().cycle();
    let mut chaos_ord = ChaosOrd::all_variants().into_iter().cycle();

    for i in 0..64 {
        eprintln!("iteration {i}");
        let key1_chaos = KeyChaos::default()
            .with_eq(chaos_eq.next().unwrap())
            .with_ord(chaos_ord.next().unwrap());

        let item = TestItem::new(i, 'a', "x", "v").with_key1_chaos(key1_chaos);
        // This may or may not work, and may even panic; we care about two
        // things:
        //
        // 1. The map shouldn't be left in an invalid state.
        // 2. UB detection with Miri.
        catch_panic(|| map.insert_unique(item.clone()));
        // iter_mut can potentially cause mutable UB.
        catch_panic(|| map.iter_mut().collect::<Vec<_>>());
        catch_panic(|| match map.entry(item.key()) {
            id_ord_map::Entry::Vacant(_) => {}
            id_ord_map::Entry::Occupied(mut entry) => {
                // This can trigger some unsafe code.
                {
                    let _mut1 = entry.get_mut();
                }
                let _mut2 = entry.into_mut();
            }
        });
        without_chaos(|| {
            map.validate(ValidateCompact::Compact, ValidateChaos::Yes)
                .unwrap_or_else(|error| {
                    panic!("iteration {i}: map invalid: {error}")
                })
        });
    }
}

#[test]
fn test_insert_unique() {
    let mut map = IdOrdMap::<TestItem>::make_new();

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

    // Add a duplicate against key2. IdOrdMap only uses key1 here, so this
    // should be allowed.
    let v3 = TestItem::new(5, 'a', "y", "v");
    map.insert_unique(v3.clone()).unwrap();

    // Add a duplicate against key1, which should error out.
    let v4 = TestItem::new(5, 'b', "x", "v");
    let error = map.insert_unique(v4.clone()).unwrap_err();
    assert_eq!(error.new_item(), &v4);

    // Iterate over the items mutably. This ensures that miri detects UB if it
    // exists.
    let items: Vec<id_ord_map::RefMut<_>> = map.iter_mut().collect();
    let e1 = &items[0];
    assert_eq!(**e1, v3);

    // Test that the RefMut Debug impl looks good.
    assert!(
        format!("{e1:?}").starts_with(
            r#"TestItem { key1: 5, key2: 'a', key3: "y", value: "v""#,
        ),
        "RefMut Debug impl should forward to TestItem",
    );

    let e2 = &*items[1];
    assert_eq!(*e2, v1);
}

#[test]
fn from_iter_unique_duplicate_key_reports_error() {
    let existing = TestItem::new(1, 'a', "x", "first");
    let new_item = TestItem::new(1, 'c', "z", "dup");
    let items = [
        existing.clone(),
        TestItem::new(2, 'b', "y", "second"),
        new_item.clone(),
    ];

    let error = IdOrdMap::<TestItem>::from_iter_unique(items).unwrap_err();
    assert_eq!(error.new_item(), &new_item);
    assert_eq!(error.duplicates(), &[existing]);
}

#[test]
fn from_iter_unique_empty_is_ok() {
    let map = IdOrdMap::<TestItem>::from_iter_unique(Vec::new())
        .expect("empty iterator yields an empty map");
    assert!(map.is_empty());
}

// Test that the unsafe block within RefMut doesn't trip up miri.
#[test]
fn test_ref_mut_aliasing() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    for i in 0..16_u8 {
        map.insert_unique(TestItem::new(i, 'a', "x", "v")).unwrap();
    }

    let mut items: Vec<_> = map.iter_mut().collect();
    for (i, item) in items.iter_mut().enumerate() {
        item.value = format!("written-{i}");
    }
    drop(items);

    for i in 0..16_u8 {
        let item = map.get(&TestKey1::new(&i)).unwrap();
        assert_eq!(item.value, format!("written-{}", i as usize));
    }
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

struct IdOrdMapMachine {
    map: IdOrdMap<TestItem>,
    naive: NaiveMap,
    compactness: ValidateCompact,
}

impl IdOrdMapMachine {
    fn check_valid(&mut self, change: CompactnessChange) {
        self.compactness = change.apply(self.compactness);
        self.map
            .validate(self.compactness, ValidateChaos::No)
            .expect("map should be valid");
    }
}

#[hegel::state_machine]
impl IdOrdMapMachine {
    #[rule]
    fn insert_unique(&mut self, tc: TestCase) {
        let item = tc.draw(test_item());
        let map_res = self.map.insert_unique(item.clone());
        let naive_res = self.naive.insert_unique(item.clone());

        assert_eq!(map_res.is_ok(), naive_res.is_ok());
        if let Err(map_err) = map_res {
            let naive_err = naive_res.unwrap_err();
            assert_eq!(map_err.new_item(), naive_err.new_item());
            assert_eq!(map_err.duplicates(), naive_err.duplicates());
        }

        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn insert_overwrite(&mut self, tc: TestCase) {
        let item = tc.draw(test_item());
        let map_dups = self.map.insert_overwrite(item.clone());
        let mut naive_dups = self.naive.insert_overwrite(item.clone());
        assert!(naive_dups.len() <= 1, "max one conflict");
        let naive_dup = naive_dups.pop();

        assert_eq!(
            map_dups, naive_dup,
            "map and naive map should agree on insert_overwrite dup"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn entry_insert_overwrite(&mut self, tc: TestCase) {
        let item = tc.draw(test_item());
        let map_res = match self.map.entry(item.key()) {
            id_ord_map::Entry::Occupied(mut entry) => {
                Some(entry.insert(item.clone()))
            }
            id_ord_map::Entry::Vacant(_) => None,
        };

        let occupied = self.naive.get1(item.key1).is_some();
        let naive_res = occupied.then(|| {
            let mut dups = self.naive.insert_overwrite(item.clone());
            assert!(dups.len() <= 1, "max one conflict");
            dups.pop().expect("occupied entry has one duplicate")
        });

        assert_eq!(
            map_res, naive_res,
            "map and naive map should agree on Entry::insert"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn entry_remove(&mut self, tc: TestCase) {
        let key = draw_lookup_key1(&tc, &self.naive);
        let map_res = match self.map.entry(TestKey1::new(&key)) {
            id_ord_map::Entry::Occupied(entry) => Some(entry.remove()),
            id_ord_map::Entry::Vacant(_) => None,
        };

        let naive_res = self.naive.remove1(key);

        assert_eq!(
            map_res, naive_res,
            "map and naive map should agree on Entry::remove"
        );
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn get(&mut self, tc: TestCase) {
        let key = draw_lookup_key1(&tc, &self.naive);
        let map_res = self.map.get(&TestKey1::new(&key));
        let naive_res = self.naive.get1(key);

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn remove(&mut self, tc: TestCase) {
        let key = draw_lookup_key1(&tc, &self.naive);
        let map_res = self.map.remove(&TestKey1::new(&key));
        let naive_res = self.naive.remove1(key);

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn first(&mut self, _: TestCase) {
        let map_res = self.map.first();
        let naive_res = self.naive.first();

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn last(&mut self, _: TestCase) {
        let map_res = self.map.last();
        let naive_res = self.naive.last();

        assert_eq!(map_res, naive_res);
    }

    #[rule]
    fn pop_first(&mut self, _: TestCase) {
        let map_res = self.map.pop_first();
        let naive_res = self.naive.pop_first();

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn pop_last(&mut self, _: TestCase) {
        let map_res = self.map.pop_last();
        let naive_res = self.naive.pop_last();

        assert_eq!(map_res, naive_res);
        self.check_valid(CompactnessChange::NoLongerCompact);
    }

    #[rule]
    fn first_entry_modify(&mut self, tc: TestCase) {
        let new_value = tc.draw(gs::text());
        match (self.map.first_entry(), self.naive.first_mut()) {
            (Some(mut entry), Some(item)) => {
                let key1 = entry.get().key1;
                entry.get_mut().value = new_value.clone();
                item.value = new_value.clone();
                assert_eq!(
                    self.map.get(&TestKey1::new(&key1)).unwrap().value,
                    new_value
                );
            }
            (None, None) => {
                // Both empty, this is fine.
            }
            _ => {
                panic!(
                    "map and naive_map should agree on first_entry/first_mut"
                );
            }
        }
        self.check_valid(CompactnessChange::NoChange);
    }

    #[rule]
    fn last_entry_modify(&mut self, tc: TestCase) {
        let new_value = tc.draw(gs::text());
        match (self.map.last_entry(), self.naive.last_mut()) {
            (Some(mut entry), Some(item)) => {
                let key1 = entry.get().key1;
                entry.get_mut().value = new_value.clone();
                item.value = new_value.clone();
                assert_eq!(
                    self.map.get(&TestKey1::new(&key1)).unwrap().value,
                    new_value
                );
            }
            (None, None) => {
                // Both empty, this is fine.
            }
            _ => {
                panic!("map and naive_map should agree on last_entry/last_mut");
            }
        }
        self.check_valid(CompactnessChange::NoChange);
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
        naive_items.sort_by(|a, b| a.key().cmp(&b.key()));
        assert_iter_eq(self.map.clone(), naive_items);
    }
}

#[hegel::test(test_cases = 512)]
fn proptest_ops(tc: TestCase) {
    let machine = IdOrdMapMachine {
        map: IdOrdMap::<TestItem>::make_new(),
        naive: NaiveMap::new_key1(),
        compactness: ValidateCompact::Compact,
    };
    hegel::stateful::run(machine, tc);
}

#[hegel::test(test_cases = 64)]
fn proptest_permutation_eq(tc: TestCase) {
    // draw_fill_batch generates unique keys so there's no need to deduplicate.
    let set = draw_fill_batch(&tc);
    let set2 = draw_shuffle(&tc, &set);

    let mut map1 = IdOrdMap::<TestItem>::make_new();
    let mut map2 = IdOrdMap::<TestItem>::make_new();
    for item in set.clone() {
        map1.insert_unique(item).expect("set is deduplicated");
    }
    for item in set2.clone() {
        map2.insert_unique(item).expect("set is deduplicated");
    }

    assert_eq_props(&map1, &map2);

    let map3 = IdOrdMap::from_iter_unique(set).unwrap();
    let map4 = IdOrdMap::from_iter_unique(set2).unwrap();
    assert_eq_props(&map1, &map3);
    assert_eq_props(&map3, &map4);
}

// Test various conditions for non-equality.
#[test]
fn test_permutation_eq_examples() {
    let mut map1 = IdOrdMap::<TestItem>::make_new();
    let mut map2 = IdOrdMap::<TestItem>::make_new();

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
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(128, 'b', "y", "x")).unwrap();
    map.get_mut(&TestKey1::new(&128)).unwrap().key1 = 2;
}

#[test]
fn entry_examples() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    let item1 = TestItem::new(0, 'a', "x", "v");

    let id_ord_map::Entry::Vacant(entry) = map.entry(item1.key()) else {
        panic!("expected VacantEntry")
    };
    let mut entry = entry.insert_entry(item1.clone());

    assert_eq!(entry.get(), &item1);
    assert_eq!(entry.get_mut().into_ref(), &item1);
    assert_eq!(entry.into_ref(), &item1);

    // Try looking up another item with the same key1.
    let item2 = TestItem::new(0, 'b', "y", "x");

    let id_ord_map::Entry::Occupied(mut entry) = map.entry(item2.key()) else {
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

    // Add another item using or_insert_ref.
    let item5 = TestItem::new(2, 'c', "z", "w");
    let item5_ref = map.entry(item5.key()).or_insert_ref(item5.clone());
    assert_eq!(item5_ref, &item5);

    // Add another item using or_insert_with_ref.
    let item6 = TestItem::new(3, 'd', "a", "b");
    let item6_ref = map.entry(item6.key()).or_insert_with_ref(|| item6.clone());
    assert_eq!(item6_ref, &item6);

    // item7 is similar to item5 except with a different value.
    let item7 = TestItem::new(2, 'c', "z", "yet-another-value");
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
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = IdOrdMap::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key());
    assert!(matches!(entry, id_ord_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert_ref(v1);
}

#[test]
#[should_panic = "key already present in map"]
fn or_insert_panics_for_present_key() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = IdOrdMap::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key());
    assert!(matches!(entry, id_ord_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    entry.or_insert(v1);
}

#[test]
#[should_panic = "key already present in map"]
fn insert_entry_panics_for_present_key() {
    let v1 = TestItem::new(0, 'a', "foo", "value");
    let mut map = IdOrdMap::make_new();
    map.insert_unique(v1.clone()).expect("insert_unique succeeded");

    let v2 = TestItem::new(1, 'a', "bar", "value");
    let entry = map.entry(v2.key());
    assert!(matches!(entry, id_ord_map::Entry::Vacant(_)));
    // Try inserting v1, which is present in the map.
    if let id_ord_map::Entry::Vacant(vacant_entry) = entry {
        vacant_entry.insert_entry(v1);
    } else {
        panic!("Expected Vacant entry");
    }
}

#[test]
fn test_retain_all() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    let original_len = map.len();
    map.retain(|_| true);

    assert_eq!(map.len(), original_len);
    assert_eq!(map.len(), 3);
    map.get(&TestKey1::new(&1)).expect("key 1 should be present");
    map.get(&TestKey1::new(&2)).expect("key 2 should be present");
    map.get(&TestKey1::new(&3)).expect("key 3 should be present");
}

#[test]
fn test_retain_none() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();

    map.retain(|_| false);

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn test_retain_value_contains() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(1, 'a', "x", "foo")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "bar")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "baz")).unwrap();
    map.insert_unique(TestItem::new(4, 'd', "w", "qux")).unwrap();

    map.retain(|item| item.value.contains('a'));

    assert_eq!(map.len(), 2);
    map.get(&TestKey1::new(&2)).expect("key 2 (bar) should be present");
    map.get(&TestKey1::new(&3)).expect("key 3 (baz) should be present");
    assert!(
        map.get(&TestKey1::new(&1)).is_none(),
        "key 1 (foo) should be removed"
    );
    assert!(
        map.get(&TestKey1::new(&4)).is_none(),
        "key 4 (qux) should be removed"
    );
}

#[test]
fn test_retain_modulo() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(0, 'a', "x", "v0")).unwrap();
    map.insert_unique(TestItem::new(1, 'b', "y", "v1")).unwrap();
    map.insert_unique(TestItem::new(2, 'c', "z", "v2")).unwrap();
    map.insert_unique(TestItem::new(3, 'd', "w", "v3")).unwrap();
    map.insert_unique(TestItem::new(4, 'e', "u", "v4")).unwrap();
    map.insert_unique(TestItem::new(5, 'f', "t", "v5")).unwrap();

    map.retain(|item| item.key1 % 3 == 1);

    assert_eq!(map.len(), 2);
    map.get(&TestKey1::new(&1)).expect("key 1 should be present");
    map.get(&TestKey1::new(&4)).expect("key 4 should be present");
    assert!(map.get(&TestKey1::new(&0)).is_none(), "key 0 should be removed");
    assert!(map.get(&TestKey1::new(&2)).is_none(), "key 2 should be removed");
    assert!(map.get(&TestKey1::new(&3)).is_none(), "key 3 should be removed");
    assert!(map.get(&TestKey1::new(&5)).is_none(), "key 5 should be removed");

    // Test with a larger map for miri coverage.
    let mut large_map = IdOrdMap::<TestItem>::make_new();
    for i in 0..32_u8 {
        large_map.insert_unique(TestItem::new(i, 'x', "y", "z")).unwrap();
    }

    large_map.retain(|item| item.key1 % 7 == 3);

    // Verify the retained items.
    for i in 0..32_u8 {
        if i % 7 == 3 {
            large_map
                .get(&TestKey1::new(&i))
                .unwrap_or_else(|| panic!("key {} should be present", i));
        } else {
            assert!(
                large_map.get(&TestKey1::new(&i)).is_none(),
                "key {} should be removed",
                i
            );
        }
    }
}

#[test]
fn test_retain_preserves_ordering() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.insert_unique(TestItem::new(5, 'a', "x", "v5")).unwrap();
    map.insert_unique(TestItem::new(1, 'b', "y", "v1")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "v3")).unwrap();
    map.insert_unique(TestItem::new(7, 'd', "w", "v7")).unwrap();
    map.insert_unique(TestItem::new(2, 'e', "u", "v2")).unwrap();

    // Retain odd keys
    map.retain(|item| item.key1 % 2 == 1);

    // Iteration should be in key order: 1, 3, 5, 7
    let keys: Vec<u8> = map.iter().map(|item| item.key1).collect();
    assert_eq!(keys, vec![1, 3, 5, 7]);
}

#[test]
fn test_retain_empty_map() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.retain(|_| true);
    assert!(map.is_empty());
}

#[test]
fn test_clear_empty_map() {
    let mut map = IdOrdMap::<TestItem>::make_new();
    map.clear();
    assert!(map.is_empty());
    map.validate(ValidateCompact::Compact, ValidateChaos::No)
        .expect("empty cleared map should be compact");
}

#[test]
fn test_clear_makes_compact() {
    let mut map = IdOrdMap::<TestItem>::make_new();

    // Add items.
    map.insert_unique(TestItem::new(1, 'a', "x", "v1")).unwrap();
    map.insert_unique(TestItem::new(2, 'b', "y", "v2")).unwrap();
    map.insert_unique(TestItem::new(3, 'c', "z", "v3")).unwrap();

    // Remove an item to make it non-compact.
    map.remove(&TestKey1::new(&2));
    map.validate(ValidateCompact::NonCompact, ValidateChaos::No)
        .expect("map should be valid but non-compact");

    // Clear should make it compact again.
    map.clear();
    assert!(map.is_empty());
    map.validate(ValidateCompact::Compact, ValidateChaos::No)
        .expect("cleared map should be compact");
}

#[test]
fn borrowed_item() {
    let mut map = IdOrdMap::<BorrowedItem>::default();
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
    assert_eq!(map.get("foo").unwrap().key1, "foo");
    assert_eq!(map.get("bar").unwrap().key1, "bar");

    // Check that we can mutably retrieve them.
    {
        let mut item1 = map.get_mut("foo").unwrap();
        item1.key2 = Cow::Borrowed(b"foo2");

        // Including reborrows.
        {
            let mut item1_reborrowed = item1.reborrow();
            item1_reborrowed.key3 = Path::new("foo2");
        }

        item1.key2 = Cow::Borrowed(b"foo3");
    }

    // Check that we can iterate over them.
    let keys: Vec<_> = map.iter().map(|item| item.key()).collect();
    assert_eq!(keys, vec!["bar", "foo"]);

    // Check that we can print a Debug representation, even within a function
    // (supporting this requires a little bit of unsafe code to get the
    // lifetimes to line up).
    fn fmt_debug(map: &IdOrdMap<BorrowedItem<'_>>) -> String {
        format!("{map:?}")
    }

    #[cfg(feature = "serde")]
    fn serialize_as_map(
        map: &IdOrdMap<BorrowedItem<'_>>,
    ) -> Result<String, iddqd_test_utils::serde_json::Error> {
        let mut out: Vec<u8> = Vec::new();
        let mut ser = iddqd_test_utils::serde_json::Serializer::new(&mut out);
        id_ord_map::IdOrdMapAsMap::serialize(map, &mut ser)?;
        Ok(String::from_utf8(out)
            .expect("serde_json should always emit valid UTF-8"))
    }

    static DEBUG_OUTPUT: &str = "{\"bar\": BorrowedItem { \
        key1: \"bar\", key2: [98, 97, 114], key3: \"bar\" }, \
        \"foo\": BorrowedItem { \
        key1: \"foo\", key2: [102, 111, 111, 51], key3: \"foo2\" }}";

    assert_eq!(format!("{map:?}"), DEBUG_OUTPUT);
    assert_eq!(fmt_debug(&map), DEBUG_OUTPUT);

    #[cfg(feature = "serde")]
    {
        let map_string = serialize_as_map(&map).unwrap();
        let deserialized: IdOrdMap<BorrowedItem<'_>> =
            iddqd_test_utils::serde_json::from_str(&map_string).unwrap();
        assert_eq!(map, deserialized);
    }

    // Try using the entry API against the borrowed item.
    fn entry_api_tests(map: &mut IdOrdMap<BorrowedItem<'_>>) {
        let entry = map.entry("foo");
        entry.or_insert(BorrowedItem {
            key1: "foo",
            key2: Cow::Borrowed(b"foo"),
            key3: Path::new("foo"),
        });

        let entry = map.entry("foo");
        entry.or_insert_with(|| BorrowedItem {
            key1: "foo",
            key2: Cow::Borrowed(b"foo"),
            key3: Path::new("foo"),
        });

        let entry = map.entry("bar");
        let entry = entry.and_modify(|mut v| {
            // IdOrdMap<BorrowedItem<'_>> is not indexed by key2, so changing
            // key2 will not cause a panic. (Changing key1 would cause a panic.)
            v.key2 = Cow::Borrowed(b"baz");
        });

        let id_ord_map::Entry::Occupied(mut entry) = entry else {
            panic!("Entry should be occupied")
        };
        let mut v = entry.get_mut();
        v.key2 = Cow::Borrowed(b"quux");
    }

    entry_api_tests(&mut map);
}

#[test]
fn borrowed_item_retain_non_static() {
    let foo_key = String::from("foo");
    let bar_key = String::from("bar");
    let foo_bytes = b"foo".to_vec();
    let bar_bytes = b"bar".to_vec();
    let foo_path = PathBuf::from("foo");
    let bar_path = PathBuf::from("bar");

    let mut map = IdOrdMap::<BorrowedItem<'_>>::default();
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
    assert!(map.get(foo_key.as_str()).is_some());
    assert!(map.get(bar_key.as_str()).is_none());
}

mod macro_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct User {
        id: u32,
        name: String,
    }

    impl IdOrdItem for User {
        type Key<'a> = u32;
        fn key(&self) -> Self::Key<'_> {
            self.id
        }
        id_upcast!();
    }

    #[test]
    fn macro_basic() {
        let map = id_ord_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 2, name: "Bob".to_string() },
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&1).unwrap().name, "Alice");
        assert_eq!(map.get(&2).unwrap().name, "Bob");
    }

    #[test]
    fn macro_empty() {
        let empty_map: IdOrdMap<User> = id_ord_map! {};
        assert!(empty_map.is_empty());
    }

    #[test]
    fn macro_without_trailing_comma() {
        let map = id_ord_map! {
            User { id: 1, name: "Alice".to_string() }
        };
        assert_eq!(map.len(), 1);
    }

    #[test]
    #[should_panic(expected = "DuplicateItem")]
    fn macro_duplicate_key() {
        let _map = id_ord_map! {
            User { id: 1, name: "Alice".to_string() },
            User { id: 1, name: "Bob".to_string() },
        };
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use crate::hegel_support::draw_random_batch;
    use hegel::TestCase;
    use iddqd::IdOrdMap;
    use iddqd_test_utils::{
        serde_utils::assert_serialize_roundtrip, test_item::TestItem,
    };

    #[hegel::test(test_cases = 256)]
    fn proptest_serialize_roundtrip(tc: TestCase) {
        let values = draw_random_batch(&tc);
        assert_serialize_roundtrip::<IdOrdMap<TestItem>>(values);
    }
}

#[cfg(feature = "proptest")]
use test_strategy::proptest;

#[cfg(feature = "proptest")]
#[proptest(cases = 16)]
fn proptest_arbitrary_map(map: IdOrdMap<TestItem>) {
    // Test that the arbitrarily generated map is valid.
    map.validate(ValidateCompact::NonCompact, ValidateChaos::No)
        .expect("map should be valid");

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

#[derive(Clone, Debug)]
struct PanickyOrdItem {
    key: u32,
}

impl IdOrdItem for PanickyOrdItem {
    type Key<'a> = iddqd_test_utils::panic_safety::PanickyKey;

    fn key(&self) -> Self::Key<'_> {
        iddqd_test_utils::panic_safety::observe_panicky_call("key");
        iddqd_test_utils::panic_safety::PanickyKey(self.key)
    }

    id_upcast!();
}

impl Drop for PanickyOrdItem {
    fn drop(&mut self) {
        iddqd_test_utils::panic_safety::observe_panicky_call("item-drop");
    }
}

mod proptest_panic_safety {
    use super::*;
    use crate::hegel_support::{MAX_PANIC_KEY, draw_armed};
    use iddqd_test_utils::panic_safety::{
        PanicSafety, PanickyKey, PanickySearchKey,
        assert_panic_fired_as_expected, assert_post_op_invariants,
        drop_unarmed, record_observation, run_armed, sorted_keys,
    };

    struct PanicMachine {
        map: IdOrdMap<PanickyOrdItem>,
        step: usize,
        pending: Option<Pending>,
    }

    struct Pending {
        label: &'static str,
        panic_safety: PanicSafety,
        armed: Option<u32>,
        panicked: bool,
        pre_state: Vec<u32>,
    }

    impl PanicMachine {
        fn armed_op(
            &mut self,
            tc: &TestCase,
            label: &'static str,
            panic_safety: PanicSafety,
            op: impl FnOnce(&mut IdOrdMap<PanickyOrdItem>),
        ) {
            // hegel runs the `#[invariant]` (which consumes `pending`) after
            // every successful rule, so `pending` must be `None` here -- if
            // not, a prior op's post-op checks were silently skipped.
            assert!(
                self.pending.is_none(),
                "previous op's post-op invariant did not run before this op",
            );
            let armed = draw_armed(tc);
            let pre_state = sorted_keys(&self.map, |item| item.key);
            let (panicked, ops) = run_armed(armed, || op(&mut self.map));
            record_observation("id_ord_map", label, ops);
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
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "insert_unique", PanicSafety::Atomic, |map| {
                drop_unarmed(map.insert_unique(PanickyOrdItem { key }));
            });
        }

        #[rule]
        fn insert_overwrite(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(
                &tc,
                "insert_overwrite",
                PanicSafety::Atomic,
                |map| {
                    drop_unarmed(map.insert_overwrite(PanickyOrdItem { key }));
                },
            );
        }

        #[rule]
        fn entry_insert_overwrite(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(
                &tc,
                "entry_insert_overwrite",
                PanicSafety::Atomic,
                |map| {
                    let entry = map.entry(PanickyKey(key));
                    if let id_ord_map::Entry::Occupied(mut entry) = entry {
                        drop_unarmed(entry.insert(PanickyOrdItem { key }));
                    }
                },
            );
        }

        #[rule]
        fn entry_remove(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "entry_remove", PanicSafety::Atomic, |map| {
                let entry = map.entry(PanickyKey(key));
                if let id_ord_map::Entry::Occupied(entry) = entry {
                    drop_unarmed(entry.remove());
                }
            });
        }

        #[rule]
        fn remove(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "remove", PanicSafety::Atomic, |map| {
                drop_unarmed(map.remove(&PanickySearchKey(key)));
            });
        }

        #[rule]
        fn get(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "get", PanicSafety::Atomic, |map| {
                let _ = map.get(&PanickySearchKey(key));
            });
        }

        #[rule]
        fn contains_key(&mut self, tc: TestCase) {
            let key = tc.draw(gs::integers::<u32>().max_value(MAX_PANIC_KEY));
            self.armed_op(&tc, "contains_key", PanicSafety::Atomic, |map| {
                let _ = map.contains_key(&PanickySearchKey(key));
            });
        }

        #[rule]
        fn pop_first(&mut self, tc: TestCase) {
            self.armed_op(&tc, "pop_first", PanicSafety::Atomic, |map| {
                drop_unarmed(map.pop_first());
            });
        }

        #[rule]
        fn pop_last(&mut self, tc: TestCase) {
            self.armed_op(&tc, "pop_last", PanicSafety::Atomic, |map| {
                drop_unarmed(map.pop_last());
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
                        let matches = item.key % modulo == rem;
                        if keep { matches } else { !matches }
                    });
                },
            );
        }

        #[rule]
        fn extend(&mut self, tc: TestCase) {
            let keys = tc.draw(
                gs::vecs(gs::integers::<u32>().max_value(MAX_PANIC_KEY))
                    .max_size(7),
            );
            // `extend` does per-step atomic operations.
            self.armed_op(&tc, "extend", PanicSafety::StepAtomic, |map| {
                map.extend(keys.into_iter().map(|key| PanickyOrdItem { key }));
            });
        }

        #[rule]
        fn fill(&mut self, tc: TestCase) {
            let keys = tc.draw(
                gs::vecs(gs::integers::<u32>().max_value(MAX_PANIC_KEY))
                    .max_size(64),
            );
            for key in keys {
                let _ = self.map.insert_unique(PanickyOrdItem { key });
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

        #[invariant]
        fn check_post_op(&mut self, _: TestCase) {
            let Some(p) = self.pending.take() else {
                self.map
                    .validate(ValidateCompact::NonCompact, ValidateChaos::No)
                    .expect("map should be valid");
                return;
            };
            let step = self.step;

            // `NonCompact` since step-atomic panics can leave compactness in an
            // indeterminate state.
            self.map
                .validate(ValidateCompact::NonCompact, ValidateChaos::No)
                .unwrap_or_else(|err| {
                    panic!(
                        "map invalid after op {step} ({}, armed: {:?}, \
                         panicked: {}): {err}",
                        p.label, p.armed, p.panicked
                    )
                });
            let post_state = sorted_keys(&self.map, |item| item.key);
            assert_post_op_invariants(
                step,
                &p.label,
                p.armed,
                p.panicked,
                p.panic_safety,
                &p.pre_state,
                &post_state,
                |&k| self.map.contains_key(&PanickySearchKey(k)),
            );
            self.step += 1;
        }
    }

    #[hegel::test(test_cases = 512)]
    fn proptest_panic_ops(tc: TestCase) {
        let map = IdOrdMap::<PanickyOrdItem>::new();
        hegel::stateful::run(PanicMachine { map, step: 0, pending: None }, tc);
    }
}
