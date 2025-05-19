// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{IdBTreeMap, IdOrdItem};
use serde::{
    ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;

/// An `IdBTreeMap` serializes to the list of items. Items are serialized in
/// order of their keys.
impl<T: IdOrdItem> Serialize for IdBTreeMap<T>
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
impl<'de, T: IdOrdItem + fmt::Debug> Deserialize<'de> for IdBTreeMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let items = Vec::<T>::deserialize(deserializer)?;
        let mut map = IdBTreeMap::new();
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }
        Ok(map)
    }
}
