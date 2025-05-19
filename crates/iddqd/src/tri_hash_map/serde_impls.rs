// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{TriHashItem, TriHashMap};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

/// A `TriHashMap` serializes to the list of entries. Entries are serialized in
/// arbitrary order.
impl<T: TriHashItem> Serialize for TriHashMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Serialize just the entries -- don't serialize the indexes. We'll
        // rebuild the indexes on deserialization.
        self.entries.serialize(serializer)
    }
}

/// The `Deserialize` impl for `TriHashMap` deserializes the list of entries and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: TriHashItem + fmt::Debug> Deserialize<'de> for TriHashMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // First, deserialize the entries.
        let entries = Vec::<T>::deserialize(deserializer)?;

        // Now build a map from scratch, inserting the entries sequentially.
        // This will catch issues with duplicates.
        let mut map = TriHashMap::new();
        for entry in entries {
            map.insert_unique(entry).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
