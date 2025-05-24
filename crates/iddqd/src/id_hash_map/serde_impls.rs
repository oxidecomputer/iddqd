use crate::{IdHashItem, IdHashMap};
use alloc::vec::Vec;
use core::{fmt, hash::BuildHasher};
use serde::{Deserialize, Serialize, Serializer};

/// A `TriHashMap` serializes to the list of items. Items are serialized in
/// arbitrary order.
impl<T: IdHashItem, S: Clone + BuildHasher> Serialize for IdHashMap<T, S>
where
    T: Serialize,
{
    fn serialize<Ser: Serializer>(
        &self,
        serializer: Ser,
    ) -> Result<Ser::Ok, Ser::Error> {
        // Serialize just the items -- don't serialize the indexes. We'll
        // rebuild the indexes on deserialization.
        self.items.serialize(serializer)
    }
}

/// The `Deserialize` impl for `TriHashMap` deserializes the list of items and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: IdHashItem + fmt::Debug, S: Clone + BuildHasher + Default>
    Deserialize<'de> for IdHashMap<T, S>
where
    T: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // First, deserialize the items.
        let items = Vec::<T>::deserialize(deserializer)?;

        // Now build a map from scratch, inserting the items sequentially.
        // This will catch issues with duplicates.
        let mut map = IdHashMap::default();
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
