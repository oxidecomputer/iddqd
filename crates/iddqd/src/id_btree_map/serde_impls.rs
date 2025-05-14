// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

use serde::{
    ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer,
};

use super::{IdBTreeMap, IdBTreeMapEntry};

/// An `IdBTreeMap` serializes to the list of entries. Entries are serialized in
/// order of their keys.
impl<T: IdBTreeMapEntry> Serialize for IdBTreeMap<T>
where
    T: Serialize,
{
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for entry in self {
            seq.serialize_element(entry)?;
        }
        seq.end()
    }
}

/// The `Deserialize` impl deserializes the list of entries, rebuilding the
/// indexes and producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<'de, T: IdBTreeMapEntry + fmt::Debug> Deserialize<'de> for IdBTreeMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<T>::deserialize(deserializer)?;
        let mut map = IdBTreeMap::new();
        for entry in entries {
            map.insert_unique(entry).map_err(serde::de::Error::custom)?;
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{assert_serialize_roundtrip, TestEntry};
    use test_strategy::proptest;

    #[proptest]
    fn proptest_serialize_roundtrip(values: Vec<TestEntry>) {
        assert_serialize_roundtrip::<IdBTreeMap<TestEntry>>(values);
    }
}
