use super::{IdOrdItem, IdOrdMap};
use core::{fmt, marker::PhantomData};
use serde_core::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
    ser::SerializeSeq,
};

/// An `IdOrdMap` serializes to the list of items. Items are serialized in
/// order of their keys.
///
/// Serializing as a list of items rather than as a map works around the lack of
/// non-string keys in formats like JSON.
///
/// # Examples
///
/// ```
/// use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
/// # use iddqd_test_utils::serde_json;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize)]
/// struct Item {
///     id: u32,
///     name: String,
///     email: String,
/// }
///
/// // This is a complex key, so it can't be a JSON map key.
/// #[derive(Eq, PartialEq, PartialOrd, Ord)]
/// struct ComplexKey<'a> {
///     id: u32,
///     email: &'a str,
/// }
///
/// impl IdOrdItem for Item {
///     type Key<'a> = ComplexKey<'a>;
///     fn key(&self) -> Self::Key<'_> {
///         ComplexKey { id: self.id, email: &self.email }
///     }
///     id_upcast!();
/// }
///
/// let mut map = IdOrdMap::<Item>::new();
/// map.insert_unique(Item {
///     id: 1,
///     name: "Alice".to_string(),
///     email: "alice@example.com".to_string(),
/// })
/// .unwrap();
///
/// // The map is serialized as a list of items in order of their keys.
/// let serialized = serde_json::to_string(&map).unwrap();
/// assert_eq!(
///     serialized,
///     r#"[{"id":1,"name":"Alice","email":"alice@example.com"}]"#,
/// );
/// ```
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
            map.insert_unique(element)
                .map_err(serde_core::de::Error::custom)?;
        }

        Ok(map)
    }
}
