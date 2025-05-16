// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{tri_hash_map, tri_upcasts, TriHashMap, TriHashMapEntry};
use std::fmt;
use test_strategy::Arbitrary;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Arbitrary)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct TestEntry {
    pub(super) key1: u8,
    pub(super) key2: char,
    pub(super) key3: String,
    pub(super) value: String,
}

impl PartialEq<&TestEntry> for TestEntry {
    fn eq(&self, other: &&TestEntry) -> bool {
        self.key1 == other.key1
            && self.key2 == other.key2
            && self.key3 == other.key3
            && self.value == other.value
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

/// Represents a map of `TestEntry` values. Used for generic tests and assertions.
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

    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
    fn into_iter(self) -> Self::IntoIter;
}

impl TestEntryMap for TriHashMap<TestEntry> {
    type RefMut<'a> = tri_hash_map::RefMut<'a, TestEntry>;
    type Iter<'a> = tri_hash_map::Iter<'a, TestEntry>;
    type IterMut<'a> = tri_hash_map::IterMut<'a, TestEntry>;
    type IntoIter = tri_hash_map::IntoIter<TestEntry>;

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

pub(crate) trait IntoRef<'a> {
    fn into_ref(self) -> &'a TestEntry;
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
