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
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};
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

/// A keys-triple sourced from a mix of "an existing item in the map" and
/// random fallback values.
///
/// Each component independently either copies a key from an item at
/// `key{1,2,3}_from % naive_map.len()` (when the map is non-empty), or falls
/// back to the random `rand_key{1,2,3}` value. This mix-and-match makes "right
/// key1, right key2, wrong key3"-style triples (and permutations thereof)
/// common in the proptest stream, which is what the `_unique` methods need to
/// be exercised on.
#[derive(Clone, Debug, Arbitrary)]
struct UniqueKeysOp {
    key1_from: Option<u8>,
    key2_from: Option<u8>,
    key3_from: Option<u8>,
    rand_key1: u8,
    rand_key2: char,
    rand_key3: String,
}

impl UniqueKeysOp {
    /// Resolves the triple against the current oracle state.
    fn resolve(&self, naive_map: &NaiveMap) -> (u8, char, String) {
        let items: Vec<&TestItem> = naive_map.iter().collect();
        let pick_from = |from: Option<u8>| -> Option<&TestItem> {
            let len = items.len();
            from.and_then(|i| {
                if len == 0 { None } else { Some(items[i as usize % len]) }
            })
        };
        let key1 = pick_from(self.key1_from)
            .map(|item| item.key1)
            .unwrap_or(self.rand_key1);
        let key2 = pick_from(self.key2_from)
            .map(|item| item.key2)
            .unwrap_or(self.rand_key2);
        let key3 = pick_from(self.key3_from)
            .map(|item| item.key3.clone())
            .unwrap_or_else(|| self.rand_key3.clone());
        (key1, key2, key3)
    }
}

#[derive(Debug, Arbitrary)]
enum Operation {
    // Make inserts a bit more common to try and fill up the map.
    #[weight(4)]
    InsertUnique(TestItem),
    #[weight(3)]
    InsertOverwrite(TestItem),
    #[weight(2)]
    Get1(u8),
    #[weight(2)]
    Get2(char),
    #[weight(2)]
    Get3(String),
    #[weight(2)]
    GetUnique(UniqueKeysOp),
    #[weight(2)]
    GetMutUnique(UniqueKeysOp),
    #[weight(2)]
    Remove1(u8),
    #[weight(2)]
    Remove2(char),
    #[weight(2)]
    Remove3(String),
    #[weight(2)]
    RemoveUnique(UniqueKeysOp),
    #[weight(2)]
    RetainValueContains(char, bool),
    #[weight(2)]
    RetainModulo(#[strategy(0..3_u8)] u8, #[strategy(1..4_u8)] u8, bool),
    #[weight(2)]
    Extend(
        #[strategy(prop::collection::vec(any::<TestItem>(), 0..16))]
        Vec<TestItem>,
    ),
    Clear,
    // `additional` is kept modest so that reservations frequently
    // exceed the current `growth_left` and so trigger hashbrown's
    // rehash path.
    Reserve(#[strategy(0..256_usize)] usize),
    TryReserve(#[strategy(0..256_usize)] usize),
    ShrinkToFit,
    ShrinkTo(#[strategy(0..256_usize)] usize),
}

impl Operation {
    fn compactness_change(&self) -> CompactnessChange {
        match self {
            Operation::InsertUnique(_)
            | Operation::Get1(_)
            | Operation::Get2(_)
            | Operation::Get3(_)
            | Operation::GetUnique(_)
            | Operation::GetMutUnique(_)
            | Operation::Reserve(_)
            | Operation::TryReserve(_) => CompactnessChange::NoChange,
            // The act of removing items, including calls to insert_overwrite,
            // can make the map non-compact.
            Operation::InsertOverwrite(_)
            | Operation::Remove1(_)
            | Operation::Remove2(_)
            | Operation::Remove3(_)
            | Operation::RemoveUnique(_)
            | Operation::RetainValueContains(_, _)
            | Operation::RetainModulo(_, _, _)
            | Operation::Extend(_) => CompactnessChange::NoLongerCompact,
            // Clear always makes the map compact (empty). Shrink
            // fully compacts the backing store, restoring the
            // `Compact` invariant.
            Operation::Clear
            | Operation::ShrinkToFit
            | Operation::ShrinkTo(_) => CompactnessChange::BecomesCompact,
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
        compactness = op.compactness_change().apply(compactness);

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
                    // The duplicates may be in any order, so sort them before
                    // comparing.
                    let mut map_err_dups = map_err.duplicates().to_vec();
                    let mut naive_err_dups = naive_err.duplicates().to_vec();
                    map_err_dups.sort();
                    naive_err_dups.sort();
                    assert_eq!(map_err_dups, naive_err_dups);
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
            Operation::GetUnique(keys) => {
                let (key1, key2, key3) = keys.resolve(&naive_map);
                let map_res = map.get_unique(
                    &TestKey1::new(&key1),
                    &TestKey2::new(key2),
                    &TestKey3::new(&key3),
                );
                let naive_res = naive_map.get_unique123(key1, key2, &key3);

                assert_eq!(map_res, naive_res);
            }
            Operation::GetMutUnique(keys) => {
                let (key1, key2, key3) = keys.resolve(&naive_map);
                let map_res = map
                    .get_mut_unique(
                        &TestKey1::new(&key1),
                        &TestKey2::new(key2),
                        &TestKey3::new(&key3),
                    )
                    .map(|r| (*r).clone());
                let naive_res =
                    naive_map.get_mut_unique123(key1, key2, &key3).cloned();

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
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
            Operation::RemoveUnique(keys) => {
                let (key1, key2, key3) = keys.resolve(&naive_map);
                let map_res = map.remove_unique(
                    &TestKey1::new(&key1),
                    &TestKey2::new(key2),
                    &TestKey3::new(&key3),
                );
                let naive_res = naive_map.remove_unique123(key1, key2, &key3);

                assert_eq!(map_res, naive_res);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::RetainValueContains(ch, equals) => {
                map.retain(|item| {
                    let contains = item.value.contains(ch);
                    if equals { contains } else { !contains }
                });
                naive_map.retain(|item| {
                    let contains = item.value.contains(ch);
                    if equals { contains } else { !contains }
                });
                map.validate(compactness).expect("map should be valid");
            }
            Operation::RetainModulo(a, b, equals) => {
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
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Extend(items) => {
                map.extend(items.clone());
                naive_map.extend(items);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Clear => {
                map.clear();
                naive_map.clear();
                map.validate(compactness).expect("map should be valid");
            }
            Operation::Reserve(additional) => {
                map.reserve(additional);
                // `reserve` has no observable effect beyond capacity; the
                // naive map has no equivalent. `validate` is the real
                // check — it iterates items and asks `find_index` for
                // each, which catches a hash-table left mis-bucketed by
                // a regrowth rehash.
                map.validate(compactness).expect("map should be valid");
            }
            Operation::TryReserve(additional) => {
                // Mirror `Reserve`; we don't assert `Ok` because the
                // allocator could (legitimately) refuse a large request,
                // and bailing on that would mask the actual regression
                // we care about (silent hash-table corruption).
                let _ = map.try_reserve(additional);
                map.validate(compactness).expect("map should be valid");
            }
            Operation::ShrinkToFit => {
                map.shrink_to_fit();
                map.validate(compactness).expect("map should be valid");
            }
            Operation::ShrinkTo(min_capacity) => {
                map.shrink_to(min_capacity);
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
    use allocator_api2::alloc::Global;
    use iddqd_test_utils::panic_safety::{
        PANIC_PROPTEST_CASES, PANIC_PROPTEST_MAX_OPS, PanicSafety,
        PanickyAlloc, PanickyOp, PanickySearchKey,
        assert_panic_fired_as_expected, assert_post_op_invariants,
        drop_unarmed, record_observation, run_armed, sorted_keys,
    };

    /// Map type used by these tests.
    ///
    /// Wraps the allocator in [`PanickyAlloc`] so the shared panic
    /// countdown can fire from inside `allocate`. That makes the shrink
    /// actions below (which would otherwise make zero observable user
    /// calls — `cached_hasher` keeps hashbrown from invoking user `Hash`)
    /// participate in the panic-injection schedule.
    type PanickyMap = TriHashMap<
        PanickyHashItem,
        iddqd::DefaultHashBuilder,
        PanickyAlloc<Global>,
    >;

    // Keys are kept in a small range so hits and misses both happen
    // frequently against a 16-ish-element map.
    #[derive(Debug, Arbitrary)]
    enum PanickyAction {
        #[weight(4)]
        InsertUnique(
            #[strategy(0..32_u32)] u32,
            #[strategy(0..32_u32)] u32,
            #[strategy(0..32_u32)] u32,
        ),
        #[weight(3)]
        InsertOverwrite(
            #[strategy(0..32_u32)] u32,
            #[strategy(0..32_u32)] u32,
            #[strategy(0..32_u32)] u32,
        ),
        #[weight(2)]
        Remove1(#[strategy(0..32_u32)] u32),
        #[weight(2)]
        Remove2(#[strategy(0..32_u32)] u32),
        #[weight(2)]
        Remove3(#[strategy(0..32_u32)] u32),
        #[weight(1)]
        Get1(#[strategy(0..32_u32)] u32),
        #[weight(1)]
        Get2(#[strategy(0..32_u32)] u32),
        #[weight(1)]
        Get3(#[strategy(0..32_u32)] u32),
        #[weight(2)]
        RetainModulo(
            #[strategy(0..3_u32)] u32,
            #[strategy(1..4_u32)] u32,
            bool,
        ),
        #[weight(2)]
        Extend(
            #[strategy(prop::collection::vec(
                (0..32_u32, 0..32_u32, 0..32_u32), 0..8,
            ))]
            Vec<(u32, u32, u32)>,
        ),
        Clear,
        ShrinkToFit,
        ShrinkTo(#[strategy(0..32_usize)] usize),
    }

    impl PanickyAction {
        /// Classify panic safety for this action.
        ///
        /// * `RetainModulo` and `Clear` loop over per-step atomic item
        ///   destruction.
        /// * `Extend` calls `HashTable::reserve` up front, which on a
        ///   tombstone-heavy map drops into hashbrown's
        ///   `rehash_in_place` — documented as not panic-safe under a
        ///   user `Hash` panic, so the proptest skips arming for it.
        /// * `ShrinkToFit` / `ShrinkTo` reorganize indexes and capacities
        ///   but never add, remove, or drop items, so the observable
        ///   set of keys is invariant — atomic in this test's sense.
        fn panic_safety(&self) -> PanicSafety {
            match self {
                PanickyAction::InsertUnique(_, _, _)
                | PanickyAction::InsertOverwrite(_, _, _)
                | PanickyAction::Remove1(_)
                | PanickyAction::Remove2(_)
                | PanickyAction::Remove3(_)
                | PanickyAction::Get1(_)
                | PanickyAction::Get2(_)
                | PanickyAction::Get3(_)
                | PanickyAction::ShrinkToFit
                | PanickyAction::ShrinkTo(_) => PanicSafety::Atomic,
                PanickyAction::RetainModulo(_, _, _)
                | PanickyAction::Extend(_)
                | PanickyAction::Clear => PanicSafety::StepAtomic,
            }
        }

        fn run(self, map: &mut PanickyMap) {
            match self {
                PanickyAction::InsertUnique(key1, key2, key3) => {
                    drop_unarmed(map.insert_unique(PanickyHashItem {
                        key1,
                        key2,
                        key3,
                    }));
                }
                PanickyAction::InsertOverwrite(key1, key2, key3) => {
                    drop_unarmed(map.insert_overwrite(PanickyHashItem {
                        key1,
                        key2,
                        key3,
                    }));
                }
                PanickyAction::Remove1(key1) => {
                    drop_unarmed(map.remove1(&PanickySearchKey(key1)));
                }
                PanickyAction::Remove2(key2) => {
                    drop_unarmed(map.remove2(&PanickySearchKey(key2)));
                }
                PanickyAction::Remove3(key3) => {
                    drop_unarmed(map.remove3(&PanickySearchKey(key3)));
                }
                PanickyAction::Get1(key1) => {
                    let _ = map.get1(&PanickySearchKey(key1));
                }
                PanickyAction::Get2(key2) => {
                    let _ = map.get2(&PanickySearchKey(key2));
                }
                PanickyAction::Get3(key3) => {
                    let _ = map.get3(&PanickySearchKey(key3));
                }
                PanickyAction::RetainModulo(rem, modulo, keep) => {
                    map.retain(|item| {
                        let matches = item.key1 % modulo == rem;
                        if keep { matches } else { !matches }
                    });
                }
                PanickyAction::Extend(triples) => {
                    map.extend(triples.into_iter().map(
                        |(key1, key2, key3)| PanickyHashItem {
                            key1,
                            key2,
                            key3,
                        },
                    ));
                }
                PanickyAction::Clear => map.clear(),
                PanickyAction::ShrinkToFit => map.shrink_to_fit(),
                PanickyAction::ShrinkTo(min_capacity) => {
                    map.shrink_to(min_capacity);
                }
            }
        }
    }

    #[proptest(cases = PANIC_PROPTEST_CASES)]
    fn proptest_panic_ops(
        #[strategy(prop::collection::vec(
            any::<PanickyOp<PanickyAction>>(), 0..PANIC_PROPTEST_MAX_OPS,
        ))]
        ops: Vec<PanickyOp<PanickyAction>>,
    ) {
        let mut map = PanickyMap::with_hasher_in(
            iddqd::DefaultHashBuilder::default(),
            PanickyAlloc::default(),
        );

        for (i, op) in ops.into_iter().enumerate() {
            let action = op.action;
            let action_label = format!("{action:?}");
            let panic_safety = action.panic_safety();
            let armed = op.armed;

            let pre_state =
                sorted_keys(&map, |item| (item.key1, item.key2, item.key3));
            let (panicked, ops) = run_armed(armed, || action.run(&mut map));
            record_observation("tri_hash_map", &action_label, ops);
            assert_panic_fired_as_expected(&action_label, armed, panicked, ops);

            // `NonCompact` since step-atomic panics leave compactness
            // in an indeterminate state.
            map.validate(ValidateCompact::NonCompact).unwrap_or_else(|err| {
                panic!(
                    "map invalid after op {i} ({action_label}, \
                     armed: {armed:?}, panicked: {panicked}): {err}"
                )
            });

            let post_state =
                sorted_keys(&map, |item| (item.key1, item.key2, item.key3));
            assert_post_op_invariants(
                i,
                &action_label,
                armed,
                panicked,
                panic_safety,
                &pre_state,
                &post_state,
                |&(k1, k2, k3)| {
                    map.contains_key1(&PanickySearchKey(k1))
                        && map.contains_key2(&PanickySearchKey(k2))
                        && map.contains_key3(&PanickySearchKey(k3))
                },
            );
        }
    }
}

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
            let occupied =
                entry.insert_entry(SimpleItem { key1: 2, key2: 'b', key3: 2 });
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
    let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
    map.insert_unique(SimpleItem { key1: 1, key2: 'a', key3: 1 }).unwrap();
    map.insert_unique(SimpleItem { key1: 2, key2: 'b', key3: 2 }).unwrap();

    match map.entry(1, 'a', 2) {
        tri_hash_map::Entry::Occupied(entry) => {
            let entry_ref = entry.get();
            assert_eq!(entry_ref.by_key1().unwrap().key1, 1);
            assert_eq!(entry_ref.by_key2().unwrap().key1, 1);
            assert_eq!(entry_ref.by_key3().unwrap().key1, 2);
            let tri_hash_map::OccupiedEntryRef::NonUnique(non_unique) =
                entry_ref
            else {
                panic!("expected non-unique entry ref");
            };
            assert_eq!(non_unique.by_key1().unwrap().key1, 1);
            assert_eq!(non_unique.by_key2().unwrap().key1, 1);
            assert_eq!(non_unique.by_key3().unwrap().key1, 2);
            let mut seen = Vec::new();
            non_unique.for_each(|item| seen.push(item.key1));
            assert_eq!(seen, vec![1, 2]);
        }
        tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
    }

    match map.entry(1, 'z', 1) {
        tri_hash_map::Entry::Occupied(entry) => {
            let entry_ref = entry.get();
            assert_eq!(entry_ref.by_key1().unwrap().key1, 1);
            assert!(entry_ref.by_key2().is_none());
            assert_eq!(entry_ref.by_key3().unwrap().key1, 1);
            let tri_hash_map::OccupiedEntryRef::NonUnique(non_unique) =
                entry_ref
            else {
                panic!("expected non-unique entry ref");
            };
            assert_eq!(non_unique.by_key1().unwrap().key1, 1);
            assert!(non_unique.by_key2().is_none());
            assert_eq!(non_unique.by_key3().unwrap().key1, 1);
            let mut seen = Vec::new();
            non_unique.for_each(|item| seen.push(item.key1));
            assert_eq!(seen, vec![1]);
        }
        tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
    }

    match map.entry(9, 'b', 1) {
        tri_hash_map::Entry::Occupied(entry) => {
            let entry_ref = entry.get();
            assert!(entry_ref.by_key1().is_none());
            assert_eq!(entry_ref.by_key2().unwrap().key1, 2);
            assert_eq!(entry_ref.by_key3().unwrap().key1, 1);
            let mut seen = Vec::new();
            entry_ref.for_each(|item| seen.push(item.key1));
            assert_eq!(seen, vec![2, 1]);
        }
        tri_hash_map::Entry::Vacant(_) => panic!("expected occupied"),
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
#[should_panic(expected = "key3 hashes do not match")]
fn entry_vacant_insert_panics_on_mismatched_key3() {
    let mut map = TriHashMap::<SimpleItem, HashBuilder, Alloc>::make_new();
    let entry = match map.entry(1, 'a', 1) {
        tri_hash_map::Entry::Vacant(entry) => entry,
        tri_hash_map::Entry::Occupied(_) => panic!("expected vacant"),
    };
    entry.insert(SimpleItem { key1: 1, key2: 'a', key3: 2 });
}
