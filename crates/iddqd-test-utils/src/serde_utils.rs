//! Serde-related test utilities.

use crate::test_item::{MapKind, TestItem, TestItemMap};
use iddqd::internal::ValidateCompact;
use serde::{Deserialize, Serialize};

pub fn assert_serialize_roundtrip<M>(values: Vec<TestItem>)
where
    M: TestItemMap + Serialize + for<'de> Deserialize<'de>,
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
    deserialized
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map is valid");

    let mut map_items = map.iter().collect::<Vec<_>>();
    let mut deserialized_items = deserialized.iter().collect::<Vec<_>>();

    match M::map_kind() {
        MapKind::Ord => {
            // No sorting required -- we expect the items to be in order.
        }
        MapKind::Hash => {
            // Sort the items, since we don't care about the order.
            map_items.sort();
            deserialized_items.sort();
        }
    }
    assert_eq!(map_items, deserialized_items, "items match");

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
            panic!(
                "unexpected error: {error}, deserialization should have succeeded"
            )
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
