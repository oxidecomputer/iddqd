use crate::{TriHashItem, TriHashMap, support::alloc::Allocator};
use core::{fmt, hash::BuildHasher, marker::PhantomData};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};

/// A `TriHashMap` serializes to the list of items. Items are serialized in
/// arbitrary order.
///
/// Serializing as a list of items rather than as a map works around the lack of
/// non-string keys in formats like JSON.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
/// # use iddqd_test_utils::serde_json;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize)]
/// struct Item {
///     id: u32,
///     name: String,
///     email: String,
///     value: usize,
/// }
///
/// // This is a complex key, so it can't be a JSON map key.
/// #[derive(Eq, Hash, PartialEq)]
/// struct ComplexKey<'a> {
///     name: &'a str,
///     email: &'a str,
/// }
///
/// impl TriHashItem for Item {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///     type K3<'a> = ComplexKey<'a>;
///     fn key1(&self) -> Self::K1<'_> {
///         self.id
///     }
///     fn key2(&self) -> Self::K2<'_> {
///         &self.name
///     }
///     fn key3(&self) -> Self::K3<'_> {
///         ComplexKey { name: &self.name, email: &self.email }
///     }
///     tri_upcast!();
/// }
///
/// let mut map = TriHashMap::<Item>::new();
/// map.insert_unique(Item {
///     id: 1,
///     name: "Alice".to_string(),
///     email: "alice@example.com".to_string(),
///     value: 42,
/// })
/// .unwrap();
///
/// // The map is serialized as a list of items.
/// let serialized = serde_json::to_string(&map).unwrap();
/// assert_eq!(
///     serialized,
///     r#"[{"id":1,"name":"Alice","email":"alice@example.com","value":42}]"#,
/// );
/// # }
/// ```
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
                self.hasher,
                self.alloc,
            ),
            None => TriHashMap::with_hasher_in(self.hasher, self.alloc),
        };

        while let Some(element) = seq.next_element()? {
            map.insert_unique(element).map_err(serde::de::Error::custom)?;
        }

        Ok(map)
    }
}
