// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{TriHashMap, TriHashMapEntry};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

/// A `TriHashMap` serializes to the list of entries. Entries are serialized in
/// arbitrary order.
impl<T: TriHashMapEntry> Serialize for TriHashMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Serialize just the entries -- don't serialize the indexes. We'll
        // rebuild the indexes on deserialization.
        self.entries.serialize(serializer)
    }
}

/// The `Deserialize` impl for `TriHashMap` deserializes the list of entries and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: TriHashMapEntry + fmt::Debug> Deserialize<'de> for TriHashMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // First, deserialize the entries.
        let entries = Vec::<T>::deserialize(deserializer)?;

        // Now build a map from scratch, inserting the entries sequentially.
        // This will catch issues with duplicates.
        let mut map = TriHashMap::new();
        for entry in entries {
            map.insert_unique(entry).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tri_hash_map::test_utils::TestEntry;
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestEntry>) {
        let mut map = TriHashMap::<TestEntry>::new();
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
        let deserialized: TriHashMap<TestEntry> =
            serde_json::from_str(&serialized).unwrap();
        deserialized.validate().expect("deserialized map is valid");

        let mut map_entries = map.entries.into_vec();
        let mut deserialized_entries = deserialized.entries.into_vec();

        // Sort the entries, since we don't care about the order.
        map_entries.sort();
        deserialized_entries.sort();
        assert_eq!(map_entries, deserialized_entries, "entries match");

        // Try deserializing the full list of values directly, and see that the
        // error reported is the same as first_error.
        //
        // Here we rely on the fact that a TriHashMap is serialized as just a
        // vector.
        let serialized = serde_json::to_string(&values).unwrap();
        let res: Result<TriHashMap<TestEntry>, _> =
            serde_json::from_str(&serialized);
        match (first_error, res) {
            (None, Ok(_)) => {} // No error, should be fine
            (Some(first_error), Ok(_)) => {
                panic!("expected error ({first_error}), but deserialization succeeded")
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
}
