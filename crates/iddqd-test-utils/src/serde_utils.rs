//! Serde-related test utilities.

use crate::test_item::{ItemMap, MapKind, TestItem};
use iddqd::internal::ValidateCompact;
use serde::Serialize;
use std::collections::BTreeMap;

pub fn assert_serialize_roundtrip<M>(values: Vec<TestItem>)
where
    M: 'static + ItemMap<TestItem> + Serialize,
    for<'a> M::K1<'a>: Serialize,
{
    let mut map = M::make_new();
    let mut first_error = None;
    for value in values.clone() {
        // Ignore errors from duplicates which are quite possible to occur
        // here, since we're just testing serialization. But store the
        // first error to ensure that deserialization returns errors.
        if let Err(error) = map.insert_unique(value)
            && first_error.is_none()
        {
            first_error = Some(error.into_owned());
        }
    }

    let serialized = serde_json::to_string(&map).unwrap();
    let serialized_as_map = M::serialize_as_map(&map).unwrap();
    let deserialized: M = M::make_deserialize_in(
        &mut serde_json::Deserializer::from_str(&serialized),
    )
    .unwrap();
    let deserialized_from_map: M = M::make_deserialize_in(
        &mut serde_json::Deserializer::from_str(&serialized_as_map),
    )
    .unwrap();
    let deserialized_as_map = M::deserialize_as_map(
        &mut serde_json::Deserializer::from_str(&serialized_as_map),
    )
    .unwrap();
    // The `AsSeq` deserializers read the sequence form, both from JSON and
    // from postcard, which is not self-describing.
    let deserialized_as_seq = M::deserialize_as_seq(
        &mut serde_json::Deserializer::from_str(&serialized),
    )
    .unwrap();
    let postcard_bytes = postcard::to_allocvec(&map).unwrap();
    let deserialized_as_seq_postcard = M::deserialize_as_seq(
        &mut postcard::Deserializer::from_bytes(&postcard_bytes),
    )
    .unwrap();
    // They read only the sequence form, so the map form must fail.
    assert!(
        M::deserialize_as_seq(&mut serde_json::Deserializer::from_str(
            &serialized_as_map,
        ))
        .is_err(),
        "AsSeq deserializer rejects the map form"
    );
    // Also check that we can deserialize into a BTreeMap (this ensures that
    // serialized_as_map is a map type).
    let deserialized_btree_map: BTreeMap<u8, TestItem> =
        serde_json::from_str(&serialized_as_map).unwrap();
    deserialized
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map is valid");
    deserialized_from_map
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map from map is valid");
    deserialized_as_map
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map from map is valid");
    deserialized_as_seq
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map from seq is valid");
    deserialized_as_seq_postcard
        .validate_(ValidateCompact::Compact)
        .expect("deserialized map from postcard is valid");

    let mut map_items = map.iter().collect::<Vec<_>>();
    let mut deserialized_items = deserialized.iter().collect::<Vec<_>>();
    let mut deserialized_from_map_items =
        deserialized_from_map.iter().collect::<Vec<_>>();
    let mut deserialized_as_map_items =
        deserialized_as_map.iter().collect::<Vec<_>>();
    let deserialized_from_btree_map_items =
        deserialized_btree_map.values().collect::<Vec<_>>();
    let mut deserialized_as_seq_items =
        deserialized_as_seq.iter().collect::<Vec<_>>();
    let mut deserialized_as_seq_postcard_items =
        deserialized_as_seq_postcard.iter().collect::<Vec<_>>();

    match M::map_kind() {
        MapKind::Ord => {
            // No sorting required -- we expect the items to be in order.
        }
        MapKind::Hash => {
            // Sort the items, since we don't care about the order.
            map_items.sort();
            deserialized_items.sort();
            deserialized_from_map_items.sort();
            deserialized_as_map_items.sort();
            deserialized_as_seq_items.sort();
            deserialized_as_seq_postcard_items.sort();
            // The B-Tree map would already be sorted.
        }
    }
    assert_eq!(map_items, deserialized_items, "items match");
    assert_eq!(deserialized_items, deserialized_from_map_items, "items match");
    assert_eq!(
        deserialized_from_map_items, deserialized_from_btree_map_items,
        "items match"
    );
    assert_eq!(
        deserialized_from_btree_map_items, deserialized_as_map_items,
        "items match"
    );
    assert_eq!(map_items, deserialized_as_seq_items, "items match");
    assert_eq!(map_items, deserialized_as_seq_postcard_items, "items match");

    // Try deserializing the full list of values directly, and see that the
    // error reported is the same as first_error.
    //
    // Here, we rely on the fact that the map is serialized as just a vector.
    let expected = first_error.map(|error| error.to_string());
    let serialized = serde_json::to_string(&values).unwrap();
    let res: Result<M, _> = M::make_deserialize_in(
        &mut serde_json::Deserializer::from_str(&serialized),
    );
    assert_json_result_matches(expected.as_deref(), res);
    let res: Result<M, _> = M::deserialize_as_seq(
        &mut serde_json::Deserializer::from_str(&serialized),
    );
    assert_json_result_matches(expected.as_deref(), res);

    // postcard reports custom errors without their message, so only check
    // that duplicates are rejected.
    let postcard_values = postcard::to_allocvec(&values).unwrap();
    let res: Result<M, _> = M::deserialize_as_seq(
        &mut postcard::Deserializer::from_bytes(&postcard_values),
    );
    assert_eq!(
        res.is_err(),
        expected.is_some(),
        "postcard deserialization fails exactly when there are duplicates"
    );
}

/// Checks a JSON deserialization result against `expected`, the error from
/// inserting the same values into a map, if there was one.
fn assert_json_result_matches<M>(
    expected: Option<&str>,
    res: Result<M, serde_json::Error>,
) {
    match (expected, res) {
        (None, Ok(_)) => {} // No error, should be fine
        (Some(expected), Ok(_)) => {
            panic!("expected error ({expected}), but deserialization succeeded")
        }
        (None, Err(error)) => {
            panic!(
                "unexpected error: {error}, deserialization should have succeeded"
            )
        }
        (Some(expected), Err(error)) => {
            // expected is the error from the map, and error is the
            // deserialization error (which should always be a custom error,
            // stored as a string).
            let actual = error.to_string();

            // Ensure that line and column numbers are reported.
            let Some((actual_prefix, _)) = actual.rsplit_once(" at line ")
            else {
                panic!(
                    "error does not contain line number at the end: {actual}"
                );
            };
            assert_eq!(actual_prefix, expected, "error matches");
        }
    }
}
