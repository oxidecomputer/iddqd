use crate::{BiHashItem, BiHashMap};
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize, Serializer};

/// A `BiHashMap` serializes to the list of items. Items are serialized in
/// arbitrary order.
impl<T: BiHashItem> Serialize for BiHashMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Serialize just the items -- don't serialize the indexes. We'll
        // rebuild the indexes on deserialization.
        self.items.serialize(serializer)
    }
}

/// The `Deserialize` impl for `BiHashMap` deserializes the list of items and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: BiHashItem + fmt::Debug> Deserialize<'de> for BiHashMap<T>
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
        let mut map = BiHashMap::new();
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
