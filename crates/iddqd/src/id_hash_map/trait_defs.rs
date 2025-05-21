use core::hash::Hash;

/// An element stored in an [`IdHashMap`].
///
/// This trait is used to define the key type for the map.
///
/// # Examples
///
/// ```
/// use iddqd::{IdHashMap, IdHashItem, id_upcast};
///
/// // Define a struct with a key.
/// #[derive(Debug, PartialEq, Eq, Hash)]
/// struct MyItem {
///     id: String,
///     value: u32,
/// }
///
/// // Implement IdHashItem for the struct.
/// impl IdHashItem for MyItem {
///     // Keys can borrow from the item.
///     type Key<'a> = &'a str;
///
///     fn key(&self) -> Self::Key<'_> {
///         &self.id
///     }
///
///     id_upcast!();
/// }
///
/// // Create an IdHashMap and insert items.
/// let mut map = IdHashMap::new();
/// map.insert_unique(MyItem { id: "foo".to_string(), value: 42 }).unwrap();
/// map.insert_unique(MyItem { id: "bar".to_string(), value: 20 }).unwrap();
/// ```
///
/// [`IdHashMap`]: crate::IdHashMap
pub trait IdHashItem {
    /// The key type.
    type Key<'a>: Eq + Hash
    where
        Self: 'a;

    /// Retrieves the key.
    fn key(&self) -> Self::Key<'_>;

    /// Upcasts the key to a shorter lifetime, in effect asserting that the
    /// lifetime `'a` on [`IdHashItem::Key`] is covariant.
    ///
    /// Typically implemented via a macro.
    fn upcast_key<'short, 'long: 'short>(
        long: Self::Key<'long>,
    ) -> Self::Key<'short>;
}
