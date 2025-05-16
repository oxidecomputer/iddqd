// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    errors::DuplicateEntry,
    id_btree_map::{self},
    id_upcast, tri_hash_map, tri_upcasts, IdBTreeMap, IdBTreeMapEntry,
    IdBTreeMapEntryMut, TriHashMap, TriHashMapEntry,
};
use proptest::{prelude::*, sample::SizeRange};
use std::fmt;
use test_strategy::Arbitrary;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Arbitrary)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct TestEntry {
    pub(crate) key1: u8,
    pub(crate) key2: char,
    pub(crate) key3: String,
    pub(crate) value: String,
}

impl PartialEq<&TestEntry> for TestEntry {
    fn eq(&self, other: &&TestEntry) -> bool {
        self.key1 == other.key1
            && self.key2 == other.key2
            && self.key3 == other.key3
            && self.value == other.value
    }
}

impl IdBTreeMapEntry for TestEntry {
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

impl IdBTreeMapEntryMut for TestEntry {
    type OwnedKey = u8;

    fn owned_key(&self) -> Self::OwnedKey {
        self.key1
    }
}

impl TriHashMapEntry for TestEntry {
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

pub(crate) enum MapKind {
    BTree,
    Hash,
}

/// Represents a map of `TestEntry` values. Used for generic tests and assertions.
#[cfg_attr(not(feature = "serde"), expect(unused))]
pub(crate) trait TestEntryMap: Clone {
    type RefMut<'a>: IntoRef<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = &'a TestEntry>
    where
        Self: 'a;
    type IterMut<'a>: Iterator<Item = Self::RefMut<'a>>
    where
        Self: 'a;
    type IntoIter: Iterator<Item = TestEntry>;

    fn map_kind() -> MapKind;
    fn new() -> Self;
    fn validate(&self, compactness: ValidateCompact) -> anyhow::Result<()>;
    fn insert_unique(
        &mut self,
        value: TestEntry,
    ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>>;
    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
    fn into_iter(self) -> Self::IntoIter;
}

impl TestEntryMap for IdBTreeMap<TestEntry> {
    type RefMut<'a> = id_btree_map::RefMut<'a, TestEntry>;
    type Iter<'a> = id_btree_map::Iter<'a, TestEntry>;
    type IterMut<'a> = id_btree_map::IterMut<'a, TestEntry>;
    type IntoIter = id_btree_map::IntoIter<TestEntry>;

    fn map_kind() -> MapKind {
        MapKind::BTree
    }

    fn new() -> Self {
        IdBTreeMap::new()
    }

    fn validate(&self, compactness: ValidateCompact) -> anyhow::Result<()> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestEntry,
    ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>> {
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

impl TestEntryMap for TriHashMap<TestEntry> {
    type RefMut<'a> = tri_hash_map::RefMut<'a, TestEntry>;
    type Iter<'a> = tri_hash_map::Iter<'a, TestEntry>;
    type IterMut<'a> = tri_hash_map::IterMut<'a, TestEntry>;
    type IntoIter = tri_hash_map::IntoIter<TestEntry>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        TriHashMap::new()
    }

    fn validate(&self, compactness: ValidateCompact) -> anyhow::Result<()> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestEntry,
    ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>> {
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

pub(crate) trait IntoRef<'a> {
    fn into_ref(self) -> &'a TestEntry;
}

impl<'a> IntoRef<'a> for id_btree_map::RefMut<'a, TestEntry> {
    fn into_ref(self) -> &'a TestEntry {
        self.into_ref()
    }
}

impl<'a> IntoRef<'a> for tri_hash_map::RefMut<'a, TestEntry> {
    fn into_ref(self) -> &'a TestEntry {
        self.into_ref()
    }
}

pub(crate) fn assert_iter_eq<M: TestEntryMap>(
    mut map: M,
    entries: Vec<&TestEntry>,
) {
    let mut iter_entries = map.iter().collect::<Vec<_>>();
    iter_entries.sort_by_key(|e| e.key1());
    assert_eq!(iter_entries, entries, ".iter() entries match naive");

    let mut iter_mut_entries =
        map.iter_mut().map(|v| v.into_ref()).collect::<Vec<_>>();
    iter_mut_entries.sort_by_key(|e| e.key1());
    assert_eq!(
        iter_mut_entries, entries,
        ".iter_mut() entries match naive ones"
    );

    let mut into_iter_entries = map.clone().into_iter().collect::<Vec<_>>();
    into_iter_entries.sort_by_key(|e| e.key1());
    assert_eq!(
        into_iter_entries, entries,
        ".into_iter() entries match naive ones"
    );
}

// Returns a pair of permutations of a set of unique entries (unique to a given
// map).
pub(crate) fn test_entry_permutation_strategy<M: TestEntryMap>(
    size: impl Into<SizeRange>,
) -> impl Strategy<Value = (Vec<TestEntry>, Vec<TestEntry>)> {
    prop::collection::vec(any::<TestEntry>(), size.into()).prop_perturb(
        |v, mut rng| {
            // It is possible (likely even) that the input vector has
            // duplicates. How can we remove them? The easiest way is to use
            // the logic that already exists to check for duplicates. Insert
            // all the entries one by one, then get the list.
            let mut map = M::new();
            for entry in v {
                // The error case here is expected -- we're actively
                // de-duping entries right now.
                _ = map.insert_unique(entry);
            }
            let set: Vec<_> = map.into_iter().collect();

            // Now shuffle the entries. This is a simple Fisher-Yates
            // shuffle (Durstenfeld variant, low to high).
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

/// Assert equality properties.
///
/// The PartialEq algorithms in this crate are not obviously symmetric or
/// reflexive, so we must ensure in our tests that they are.
#[allow(clippy::eq_op)]
pub(crate) fn assert_eq_props<T: Eq + fmt::Debug>(a: T, b: T) {
    assert_eq!(a, a, "a == a");
    assert_eq!(b, b, "b == b");
    assert_eq!(a, b, "a == b");
    assert_eq!(b, a, "b == a");
}

/// Assert inequality properties.
///
/// The PartialEq algorithms in this crate are not obviously symmetric or
/// reflexive, so we must ensure in our tests that they are.
#[allow(clippy::eq_op)]
pub(crate) fn assert_ne_props<T: Eq + fmt::Debug>(a: T, b: T) {
    // Also check reflexivity while we're here.
    assert_eq!(a, a, "a == a");
    assert_eq!(b, b, "b == b");
    assert_ne!(a, b, "a != b");
    assert_ne!(b, a, "b != a");
}

/// For validation, indicate whether we expect integer tables to be compact
/// (have all values in the range 0..table.len()).
///
/// Maps are expected to be compact if no remove operations were performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidateCompact {
    Compact,
    NonCompact,
}
