// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Serde-related test utilities.

use super::{MapKind, TestEntry, TestEntryMap};
use serde::{Deserialize, Serialize};

pub(crate) fn assert_serialize_roundtrip<M>(values: Vec<TestEntry>)
where
    M: TestEntryMap + Serialize + for<'de> Deserialize<'de>,
{
    let mut map = M::new();
    let mut first_error = None;
    for value in values.clone() {
        // Ignore errors from duplicates which are quite possible to occur
        // here, since we're just testing serialization. But store the
        // first error to ensure that deserialization returns errors.
        if let Err(error) = map.insert_unique(value) {
            if first_error.is_none() {
                first_error = Some(error.into_owned());
            }
        }
    }

    let serialized = serde_json::to_string(&map).unwrap();
    let deserialized: M = serde_json::from_str(&serialized).unwrap();
    deserialized.validate().expect("deserialized map is valid");

    let mut map_entries = map.iter().collect::<Vec<_>>();
    let mut deserialized_entries = deserialized.iter().collect::<Vec<_>>();

    match M::map_kind() {
        MapKind::BTree => {
            // No sorting required -- we expect the entries to be in order.
        }
        MapKind::Hash => {
            // Sort the entries, since we don't care about the order.
            map_entries.sort();
            deserialized_entries.sort();
        }
    }
    assert_eq!(map_entries, deserialized_entries, "entries match");

    // Try deserializing the full list of values directly, and see that the
    // error reported is the same as first_error.
    //
    // Here, we rely on the fact that the map is serialized as just a vector.
    let serialized = serde_json::to_string(&values).unwrap();
    let res: Result<M, _> = serde_json::from_str(&serialized);
    match (first_error, res) {
        (None, Ok(_)) => {} // No error, should be fine
        (Some(first_error), Ok(_)) => {
            panic!(
                "expected error ({first_error}), but deserialization succeeded"
            )
        }
        (None, Err(error)) => {
            panic!("unexpected error: {error}, deserialization should have succeeded")
        }
        (Some(first_error), Err(error)) => {
            // first_error is the error from the map, and error is the
            // deserialization error (which should always be a custom
            // error, stored as a string).
            let expected = first_error.to_string();
            let actual = error.to_string();
            assert_eq!(actual, expected, "error matches");
        }
    }
}
