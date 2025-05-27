use crate::{IdHashItem, IdHashMap, support::alloc::Allocator};
use alloc::vec::Vec;
use core::{fmt, hash::BuildHasher};
use serde::{Deserialize, Serialize, Serializer};

/// An `IdHashMap` serializes to the list of items. Items are serialized in
/// arbitrary order.
impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> Serialize
    for IdHashMap<T, S, A>
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

/// The `Deserialize` impl for `IdHashMap` deserializes the list of items and
/// then rebuilds the indexes, producing an error if there are any duplicates.
///
/// The `fmt::Debug` bound on `T` ensures better error reporting.
impl<
    'de,
    T: IdHashItem + fmt::Debug,
    S: Clone + BuildHasher + Default,
    A: Default + Clone + Allocator,
> Deserialize<'de> for IdHashMap<T, S, A>
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
        let mut map = IdHashMap::with_capacity_and_hasher_in(
            items.len(),
            S::default(),
            A::default(),
        );
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}

impl<
    'de,
    T: IdHashItem + fmt::Debug + Deserialize<'de>,
    S: Default + Clone + BuildHasher,
    A: Clone + Allocator,
> IdHashMap<T, S, A>
{
    /// Deserializes from a list of items, allocating new storage within the
    /// provided allocator.
    pub fn deserialize_in<D: serde::Deserializer<'de>>(
        deserializer: D,
        alloc: A,
    ) -> Result<Self, D::Error> {
        Self::deserialize_with_hasher_in(deserializer, S::default(), alloc)
    }

    /// Deserializes from a list of items, with the given hasher, and allocating
    /// new storage within the provided allocator.
    pub fn deserialize_with_hasher_in<D: serde::Deserializer<'de>>(
        deserializer: D,
        hasher: S,
        alloc: A,
    ) -> Result<Self, D::Error> {
        // First, deserialize the items.
        let items = Vec::<T>::deserialize(deserializer)?;

        // Now build a map from scratch, inserting the items sequentially.
        // This will catch issues with duplicates.
        let mut map =
            IdHashMap::with_capacity_and_hasher_in(items.len(), hasher, alloc);
        for item in items {
            map.insert_unique(item).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
