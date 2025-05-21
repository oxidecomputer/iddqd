use super::{IdOrdItem, IdOrdMap};
use alloc::vec::Vec;
use core::fmt;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq,
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
        let items = Vec::<T>::deserialize(deserializer)?;
        let mut map = IdOrdMap::new();
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }
        Ok(map)
    }
}
