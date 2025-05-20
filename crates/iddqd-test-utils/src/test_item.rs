// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use iddqd::{
    errors::DuplicateItem,
    id_btree_map, id_hash_map, id_upcast,
    internal::{ValidateCompact, ValidationError},
    tri_hash_map, tri_upcasts, IdBTreeMap, IdHashItem, IdHashMap, IdOrdItem,
    IdOrdItemMut, TriHashItem, TriHashMap,
};
use proptest::{prelude::*, sample::SizeRange};
use test_strategy::Arbitrary;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Arbitrary)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestItem {
    pub key1: u8,
    pub key2: char,
    pub key3: String,
    pub value: String,
}

impl PartialEq<&TestItem> for TestItem {
    fn eq(&self, other: &&TestItem) -> bool {
        self.key1 == other.key1
            && self.key2 == other.key2
            && self.key3 == other.key3
            && self.value == other.value
    }
}

impl IdHashItem for TestItem {
    // A bit weird to return a reference to a u8, but this makes sure
    // reference-based keys work properly.
    type Key<'a>
        = &'a u8
    where
        Self: 'a;

    fn key(&self) -> Self::Key<'_> {
        &self.key1
    }

    id_upcast!();
}

impl IdOrdItem for TestItem {
    // A bit weird to return a reference to a u8, but this makes sure
    // reference-based keys work properly.
    type Key<'a>
        = &'a u8
    where
        Self: 'a;

    fn key(&self) -> Self::Key<'_> {
        &self.key1
    }

    id_upcast!();
}

impl IdOrdItemMut for TestItem {
    type OwnedKey = u8;

    fn owned_key(&self) -> Self::OwnedKey {
        self.key1
    }
}

impl TriHashItem for TestItem {
    // These types are chosen to represent various kinds of keys in the
    // proptest below.
    //
    // We use u8 since there can only be 256 values, increasing the
    // likelihood of collisions in the proptest below.
    type K1<'a> = u8;
    // char is chosen because the Arbitrary impl for it is biased towards
    // ASCII, increasing the likelihood of collisions.
    type K2<'a> = char;
    // &str is a generally open-ended type that probably won't have many
    // collisions.
    type K3<'a> = &'a str;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    fn key3(&self) -> Self::K3<'_> {
        self.key3.as_str()
    }

    tri_upcasts!();
}

pub enum MapKind {
    BTree,
    Hash,
}

/// Represents a map of `TestEntry` values. Used for generic tests and assertions.
pub trait TestItemMap: Clone {
    type RefMut<'a>: IntoRef<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = &'a TestItem>
    where
        Self: 'a;
    type IterMut<'a>: Iterator<Item = Self::RefMut<'a>>
    where
        Self: 'a;
    type IntoIter: Iterator<Item = TestItem>;

    fn map_kind() -> MapKind;
    fn new() -> Self;
    fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError>;
    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>>;
    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
    fn into_iter(self) -> Self::IntoIter;
}

impl TestItemMap for IdHashMap<TestItem> {
    type RefMut<'a> = id_hash_map::RefMut<'a, TestItem>;
    type Iter<'a> = id_hash_map::Iter<'a, TestItem>;
    type IterMut<'a> = id_hash_map::IterMut<'a, TestItem>;
    type IntoIter = id_hash_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        IdHashMap::new()
    }

    fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

impl TestItemMap for IdBTreeMap<TestItem> {
    type RefMut<'a> = id_btree_map::RefMut<'a, TestItem>;
    type Iter<'a> = id_btree_map::Iter<'a, TestItem>;
    type IterMut<'a> = id_btree_map::IterMut<'a, TestItem>;
    type IntoIter = id_btree_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::BTree
    }

    fn new() -> Self {
        IdBTreeMap::new()
    }

    fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

impl TestItemMap for TriHashMap<TestItem> {
    type RefMut<'a> = tri_hash_map::RefMut<'a, TestItem>;
    type Iter<'a> = tri_hash_map::Iter<'a, TestItem>;
    type IterMut<'a> = tri_hash_map::IterMut<'a, TestItem>;
    type IntoIter = tri_hash_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        TriHashMap::new()
    }

    fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

pub trait IntoRef<'a> {
    fn into_ref(self) -> &'a TestItem;
}

impl<'a> IntoRef<'a> for id_hash_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

impl<'a> IntoRef<'a> for id_btree_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

impl<'a> IntoRef<'a> for tri_hash_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

pub fn assert_iter_eq<M: TestItemMap>(mut map: M, items: Vec<&TestItem>) {
    let mut iter = map.iter().collect::<Vec<_>>();
    iter.sort_by_key(|e| e.key1());
    assert_eq!(iter, items, ".iter() items match naive ones");

    let mut iter_mut = map.iter_mut().map(|v| v.into_ref()).collect::<Vec<_>>();
    iter_mut.sort_by_key(|e| e.key1());
    assert_eq!(iter_mut, items, ".iter_mut() items match naive ones");

    let mut into_iter = map.clone().into_iter().collect::<Vec<_>>();
    into_iter.sort_by_key(|e| e.key1());
    assert_eq!(into_iter, items, ".into_iter() items match naive ones");
}

// Returns a pair of permutations of a set of unique items (unique to a given
// map).
pub fn test_item_permutation_strategy<M: TestItemMap>(
    size: impl Into<SizeRange>,
) -> impl Strategy<Value = (Vec<TestItem>, Vec<TestItem>)> {
    prop::collection::vec(any::<TestItem>(), size.into()).prop_perturb(
        |v, mut rng| {
            // It is possible (likely even) that the input vector has
            // duplicates. How can we remove them? The easiest way is to use
            // the logic that already exists to check for duplicates. Insert
            // all the items one by one, then get the list.
            let mut map = M::new();
            for item in v {
                // The error case here is expected -- we're actively de-duping
                // items right now.
                _ = map.insert_unique(item);
            }
            let set: Vec<_> = map.into_iter().collect();

            // Now shuffle the items. This is a simple Fisher-Yates shuffle
            // (Durstenfeld variant, low to high).
            let mut set2 = set.clone();
            if set.len() < 2 {
                return (set, set2);
            }
            for i in 0..set2.len() - 2 {
                let j = rng.gen_range(i..set2.len());
                set2.swap(i, j);
            }

            (set, set2)
        },
    )
}
