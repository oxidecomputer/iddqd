// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

use crate::{tri_upcasts, TriHashMapEntry};
use test_strategy::Arbitrary;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Arbitrary)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct TestEntry {
    pub(super) key1: u8,
    pub(super) key2: char,
    pub(super) key3: String,
    pub(super) value: String,
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
