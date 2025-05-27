use super::{IdOrdItem, IdOrdMap};
use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
    ser::SerializeSeq,
};

/// An `IdOrdMap` serializes to the list of items. Items are serialized in
/// order of their keys.
impl<T: IdOrdItem> Serialize for IdOrdMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for item in self {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}

/// The `Deserialize` impl deserializes the list of items, rebuilding the
/// indexes and producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: IdOrdItem + fmt::Debug> Deserialize<'de> for IdOrdMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(SeqVisitor { _marker: PhantomData })
    }
}

struct SeqVisitor<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for SeqVisitor<T>
where
    T: IdOrdItem + Deserialize<'de> + fmt::Debug,
{
    type Value = IdOrdMap<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence of items representing an IdOrdMap")
    }

    fn visit_seq<Access>(
        self,
        mut seq: Access,
    ) -> Result<Self::Value, Access::Error>
    where
        Access: SeqAccess<'de>,
    {
        let mut map = match seq.size_hint() {
            Some(size) => IdOrdMap::with_capacity(size),
            None => IdOrdMap::new(),
        };

        while let Some(element) = seq.next_element()? {
            map.insert_unique(element).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
