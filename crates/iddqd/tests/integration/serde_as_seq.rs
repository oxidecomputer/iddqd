//! The `AsSeq` marker types deserialize only from a sequence, so they work
//! with formats that are not self-describing, such as postcard.

use iddqd::{
    BiHashItem, BiHashMap, IdHashItem, IdHashMap, IdOrdItem, IdOrdMap,
    TriHashItem, TriHashMap, bi_hash_map::BiHashMapAsSeq, bi_upcast,
    id_hash_map::IdHashMapAsSeq, id_ord_map::IdOrdMapAsSeq, id_upcast,
    tri_hash_map::TriHashMapAsSeq, tri_upcast,
};
use iddqd_test_utils::serde_json;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Item {
    id: u32,
    name: String,
    email: String,
}

impl IdOrdItem for Item {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

impl IdHashItem for Item {
    type Key<'a> = u32;
    fn key(&self) -> Self::Key<'_> {
        self.id
    }
    id_upcast!();
}

impl BiHashItem for Item {
    type K1<'a> = u32;
    type K2<'a> = &'a str;
    fn key1(&self) -> Self::K1<'_> {
        self.id
    }
    fn key2(&self) -> Self::K2<'_> {
        &self.name
    }
    bi_upcast!();
}

impl TriHashItem for Item {
    type K1<'a> = u32;
    type K2<'a> = &'a str;
    type K3<'a> = &'a str;
    fn key1(&self) -> Self::K1<'_> {
        self.id
    }
    fn key2(&self) -> Self::K2<'_> {
        &self.name
    }
    fn key3(&self) -> Self::K3<'_> {
        &self.email
    }
    tri_upcast!();
}

fn items() -> Vec<Item> {
    vec![
        Item { id: 1, name: "a".to_owned(), email: "a@example.com".to_owned() },
        Item { id: 2, name: "b".to_owned(), email: "b@example.com".to_owned() },
    ]
}

#[derive(Debug, Serialize, Deserialize)]
struct Container {
    #[serde(with = "BiHashMapAsSeq")]
    bi: BiHashMap<Item>,
    #[serde(with = "IdHashMapAsSeq")]
    hash: IdHashMap<Item>,
    #[serde(with = "IdOrdMapAsSeq")]
    ord: IdOrdMap<Item>,
    #[serde(with = "TriHashMapAsSeq")]
    tri: TriHashMap<Item>,
}

fn container() -> Container {
    Container {
        bi: items().into_iter().collect(),
        hash: items().into_iter().collect(),
        ord: items().into_iter().collect(),
        tri: items().into_iter().collect(),
    }
}

fn sorted(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by_key(|item| item.id);
    items
}

#[test]
fn postcard_fields_round_trip() {
    let bytes = postcard::to_allocvec(&container()).unwrap();
    let back: Container = postcard::from_bytes(&bytes).unwrap();

    assert_eq!(sorted(back.bi.into_iter().collect()), items());
    assert_eq!(sorted(back.hash.into_iter().collect()), items());
    assert_eq!(back.ord.into_iter().collect::<Vec<_>>(), items());
    assert_eq!(sorted(back.tri.into_iter().collect()), items());
}

#[test]
fn postcard_top_level_round_trip() {
    let ord: IdOrdMap<Item> = items().into_iter().collect();
    let bytes = postcard::to_allocvec(&ord).unwrap();
    let back = IdOrdMapAsSeq::<Item>::deserialize(
        &mut postcard::Deserializer::from_bytes(&bytes),
    )
    .unwrap();
    assert_eq!(back.into_iter().collect::<Vec<_>>(), items());

    let hash: IdHashMap<Item> = items().into_iter().collect();
    let bytes = postcard::to_allocvec(&hash).unwrap();
    let back = IdHashMapAsSeq::<Item>::deserialize(
        &mut postcard::Deserializer::from_bytes(&bytes),
    )
    .unwrap();
    assert_eq!(sorted(back.into_iter().collect()), items());

    let bi: BiHashMap<Item> = items().into_iter().collect();
    let bytes = postcard::to_allocvec(&bi).unwrap();
    let back = BiHashMapAsSeq::<Item>::deserialize(
        &mut postcard::Deserializer::from_bytes(&bytes),
    )
    .unwrap();
    assert_eq!(sorted(back.into_iter().collect()), items());

    let tri: TriHashMap<Item> = items().into_iter().collect();
    let bytes = postcard::to_allocvec(&tri).unwrap();
    let back = TriHashMapAsSeq::<Item>::deserialize(
        &mut postcard::Deserializer::from_bytes(&bytes),
    )
    .unwrap();
    assert_eq!(sorted(back.into_iter().collect()), items());
}

/// The `AsSeq` types write the same bytes as the default `Serialize` impls,
/// so data written either way reads back through them.
#[test]
fn postcard_bytes_match_default_serialize() {
    #[derive(Serialize)]
    struct Wrapper {
        #[serde(with = "IdOrdMapAsSeq")]
        ord: IdOrdMap<Item>,
    }

    let ord: IdOrdMap<Item> = items().into_iter().collect();
    let default_bytes = postcard::to_allocvec(&ord).unwrap();
    let as_seq_bytes = postcard::to_allocvec(&Wrapper { ord }).unwrap();
    assert_eq!(as_seq_bytes, default_bytes);
}

#[test]
fn postcard_rejects_duplicates() {
    let mut duplicated = items();
    duplicated.push(duplicated[0].clone());
    let bytes = postcard::to_allocvec(&duplicated).unwrap();
    let result = IdOrdMapAsSeq::<Item>::deserialize(
        &mut postcard::Deserializer::from_bytes(&bytes),
    );
    assert!(result.is_err(), "duplicate keys must be rejected");
}

#[test]
fn json_reads_sequences_only() {
    let json = serde_json::to_string(&container()).unwrap();
    let back: Container = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ord.into_iter().collect::<Vec<_>>(), items());

    let map_form = r#"{"1":{"id":1,"name":"a","email":"a@example.com"}}"#;
    let mut de = serde_json::Deserializer::from_str(map_form);
    let err = IdOrdMapAsSeq::<Item>::deserialize(&mut de).unwrap_err();
    assert!(
        err.to_string()
            .contains("a sequence of items representing an IdOrdMap"),
        "unexpected error: {err}",
    );
}
