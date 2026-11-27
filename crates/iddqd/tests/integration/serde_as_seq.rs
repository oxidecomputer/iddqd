//! The `AsSeq` marker types with serde's `with` attribute. Roundtrips through
//! JSON and postcard for every map type are covered by
//! `assert_serialize_roundtrip`.

use iddqd::{
    BiHashMap, IdHashMap, IdOrdMap, TriHashMap, bi_hash_map::BiHashMapAsSeq,
    id_hash_map::IdHashMapAsSeq, id_ord_map::IdOrdMapAsSeq,
    tri_hash_map::TriHashMapAsSeq,
};
use iddqd_test_utils::test_item::TestItem;
use serde::{Deserialize, Serialize};

fn items() -> Vec<TestItem> {
    vec![
        TestItem::new(1, 'a', "x", "first"),
        TestItem::new(2, 'b', "y", "second"),
    ]
}

#[derive(Debug, Serialize, Deserialize)]
struct Container {
    #[serde(with = "BiHashMapAsSeq")]
    bi: BiHashMap<TestItem>,
    #[serde(with = "IdHashMapAsSeq")]
    hash: IdHashMap<TestItem>,
    #[serde(with = "IdOrdMapAsSeq")]
    ord: IdOrdMap<TestItem>,
    #[serde(with = "TriHashMapAsSeq")]
    tri: TriHashMap<TestItem>,
}

fn sorted(mut items: Vec<TestItem>) -> Vec<TestItem> {
    items.sort();
    items
}

#[test]
fn postcard_fields_round_trip() {
    let container = Container {
        bi: items().into_iter().collect(),
        hash: items().into_iter().collect(),
        ord: items().into_iter().collect(),
        tri: items().into_iter().collect(),
    };
    let bytes = postcard::to_allocvec(&container).unwrap();
    let back: Container = postcard::from_bytes(&bytes).unwrap();

    assert_eq!(sorted(back.bi.into_iter().collect()), items());
    assert_eq!(sorted(back.hash.into_iter().collect()), items());
    assert_eq!(back.ord.into_iter().collect::<Vec<_>>(), items());
    assert_eq!(sorted(back.tri.into_iter().collect()), items());
}

/// The `AsSeq` types write the same bytes as the default `Serialize` impls,
/// so data written either way reads back through them.
#[test]
fn postcard_bytes_match_default_serialize() {
    #[derive(Serialize)]
    struct Wrapper {
        #[serde(with = "IdOrdMapAsSeq")]
        ord: IdOrdMap<TestItem>,
    }

    let ord: IdOrdMap<TestItem> = items().into_iter().collect();
    let default_bytes = postcard::to_allocvec(&ord).unwrap();
    let as_seq_bytes = postcard::to_allocvec(&Wrapper { ord }).unwrap();
    assert_eq!(as_seq_bytes, default_bytes);
}
