use crate::{TriHashItem, TriHashMap, support::alloc::Allocator};
use core::{fmt, hash::BuildHasher, marker::PhantomData};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};

/// A `TriHashMap` serializes to the list of items. Items are serialized in
/// arbitrary order.
impl<T: TriHashItem, S: Clone + BuildHasher, A: Allocator> Serialize
    for TriHashMap<T, S, A>
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
impl<
    'de,
    T: TriHashItem + fmt::Debug,
    S: Clone + BuildHasher + Default,
    A: Default + Clone + Allocator,
> Deserialize<'de> for TriHashMap<T, S, A>
where
    T: Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(SeqVisitor {
            _marker: PhantomData,
            hasher: S::default(),
            alloc: A::default(),
        })
    }
}
impl<
    'de,
    T: TriHashItem + fmt::Debug + Deserialize<'de>,
    S: Clone + BuildHasher,
    A: Clone + Allocator,
> TriHashMap<T, S, A>
{
    /// Deserializes from a list of items, allocating new storage within the
    /// provided allocator.
    pub fn deserialize_in<D: serde::Deserializer<'de>>(
        deserializer: D,
        alloc: A,
    ) -> Result<Self, D::Error>
    where
        S: Default,
    {
        deserializer.deserialize_seq(SeqVisitor {
            _marker: PhantomData,
            hasher: S::default(),
            alloc,
        })
    }

    /// Deserializes from a list of items, with the given hasher, using the
    /// default allocator.
    pub fn deserialize_with_hasher<D: serde::Deserializer<'de>>(
        deserializer: D,
        hasher: S,
    ) -> Result<Self, D::Error>
    where
        A: Default,
    {
        deserializer.deserialize_seq(SeqVisitor {
            _marker: PhantomData,
            hasher,
            alloc: A::default(),
        })
    }

    /// Deserializes from a list of items, with the given hasher, and allocating
    /// new storage within the provided allocator.
    pub fn deserialize_with_hasher_in<D: serde::Deserializer<'de>>(
        deserializer: D,
        hasher: S,
        alloc: A,
    ) -> Result<Self, D::Error> {
        // First, deserialize the items.
        deserializer.deserialize_seq(SeqVisitor {
            _marker: PhantomData,
            hasher,
            alloc,
        })
    }
}

struct SeqVisitor<T, S, A> {
    _marker: PhantomData<fn() -> T>,
    hasher: S,
    alloc: A,
}

impl<'de, T, S, A> Visitor<'de> for SeqVisitor<T, S, A>
where
    T: TriHashItem + Deserialize<'de> + fmt::Debug,
    S: Clone + BuildHasher,
    A: Clone + Allocator,
{
    type Value = TriHashMap<T, S, A>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence of items representing a TriHashMap")
    }

    fn visit_seq<Access>(
        self,
        mut seq: Access,
    ) -> Result<Self::Value, Access::Error>
    where
        Access: SeqAccess<'de>,
    {
        let mut map = match seq.size_hint() {
            Some(size) => TriHashMap::with_capacity_and_hasher_in(
                size,
                self.hasher.clone(),
                self.alloc.clone(),
            ),
            None => TriHashMap::with_hasher_in(
                self.hasher.clone(),
                self.alloc.clone(),
            ),
        };

        while let Some(element) = seq.next_element()? {
            map.insert_unique(element).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
