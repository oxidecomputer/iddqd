use core::hash::Hash;

/// An element stored in an [`IdHashMap`].
///
/// This trait is used to define the key type for the map.
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
