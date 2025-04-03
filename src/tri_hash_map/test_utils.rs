// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::TriHashMapEntry;
use test_strategy::Arbitrary;

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary)]
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
}
