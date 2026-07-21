use iddqd::{
    BiHashMap, IdHashMap, IdOrdMap, TriHashMap, bi_hash_map::BiHashMapAsMap,
    id_hash_map::IdHashMapAsMap, id_ord_map::IdOrdMapAsMap,
    tri_hash_map::TriHashMapAsMap,
};
use iddqd_test_utils::test_item::TestItem;
use serde::{
    Deserialize,
    de::{
        DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor,
        value::Error,
    },
};

#[derive(Clone, Copy)]
enum Shape {
    Seq,
    Map,
}

struct LyingDeserializer(Shape);

impl<'de> Deserializer<'de> for LyingDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Shape::Seq => visitor.visit_seq(LyingAccess),
            Shape::Map => visitor.visit_map(LyingAccess),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct LyingAccess;

impl<'de> SeqAccess<'de> for LyingAccess {
    type Error = Error;

    fn next_element_seed<T>(
        &mut self,
        _seed: T,
    ) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        Ok(None)
    }

    fn size_hint(&self) -> Option<usize> {
        // This would cause a crash if we didn't cap the size hint to a
        // reasonable value.
        Some(usize::MAX)
    }
}

impl<'de> MapAccess<'de> for LyingAccess {
    type Error = Error;

    fn next_key_seed<K>(
        &mut self,
        _seed: K,
    ) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        Ok(None)
    }

    fn next_value_seed<V>(&mut self, _seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        unreachable!("next_key_seed always returns None")
    }

    fn size_hint(&self) -> Option<usize> {
        // This would cause a crash if we didn't cap the size hint to a
        // reasonable value.
        Some(usize::MAX)
    }
}

/// Test that a very large size_hint is not trusted, and that the preallocated
/// size is capped to a reasonable amount instead.
#[test]
fn huge_size_hint_does_not_preallocate() {
    for shape in [Shape::Seq, Shape::Map] {
        let map = IdHashMap::<TestItem>::deserialize(LyingDeserializer(shape))
            .expect("deserialized empty IdHashMap");
        assert_eq!(map.len(), 0);

        let map = IdOrdMap::<TestItem>::deserialize(LyingDeserializer(shape))
            .expect("deserialized empty IdOrdMap");
        assert_eq!(map.len(), 0);

        let map = BiHashMap::<TestItem>::deserialize(LyingDeserializer(shape))
            .expect("deserialized empty BiHashMap");
        assert_eq!(map.len(), 0);

        let map = TriHashMap::<TestItem>::deserialize(LyingDeserializer(shape))
            .expect("deserialized empty TriHashMap");
        assert_eq!(map.len(), 0);
    }

    let map =
        IdHashMapAsMap::<TestItem>::deserialize(LyingDeserializer(Shape::Map))
            .expect("deserialized empty IdHashMap from map");
    assert_eq!(map.len(), 0);

    let map =
        IdOrdMapAsMap::<TestItem>::deserialize(LyingDeserializer(Shape::Map))
            .expect("deserialized empty IdOrdMap from map");
    assert_eq!(map.len(), 0);

    let map =
        BiHashMapAsMap::<TestItem>::deserialize(LyingDeserializer(Shape::Map))
            .expect("deserialized empty BiHashMap from map");
    assert_eq!(map.len(), 0);

    let map =
        TriHashMapAsMap::<TestItem>::deserialize(LyingDeserializer(Shape::Map))
            .expect("deserialized empty TriHashMap from map");
    assert_eq!(map.len(), 0);
}
