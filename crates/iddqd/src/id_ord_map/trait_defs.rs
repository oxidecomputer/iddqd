//! Trait definitions for `IdOrdMap`.

use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::hash::Hash;

/// An element stored in an [`IdOrdMap`].
///
/// This trait is used to define the key type for the map.
///
/// # Examples
///
/// ```
/// use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
///
/// // Define a struct with a key.
/// #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
/// struct MyItem {
///     id: String,
///     value: u32,
/// }
///
/// // Implement IdOrdItem for the struct.
/// impl IdOrdItem for MyItem {
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
/// // Create an IdOrdMap and insert items.
/// let mut map = IdOrdMap::new();
/// map.insert_unique(MyItem { id: "foo".to_string(), value: 42 }).unwrap();
/// map.insert_unique(MyItem { id: "bar".to_string(), value: 20 }).unwrap();
/// ```
///
/// [`IdOrdMap`]: crate::IdOrdMap
pub trait IdOrdItem {
    /// The key type.
    type Key<'a>: Ord
    where
        Self: 'a;

    /// Retrieves the key.
    fn key(&self) -> Self::Key<'_>;

    /// Upcasts the key to a shorter lifetime, in effect asserting that the
    /// lifetime `'a` on [`IdOrdItem::Key`] is covariant.
    ///
    /// Typically implemented via the [`id_upcast`] macro.
    ///
    /// [`id_upcast`]: crate::id_upcast
    fn upcast_key<'short, 'long: 'short>(
        long: Self::Key<'long>,
    ) -> Self::Key<'short>;
}

macro_rules! impl_for_ref {
    ($type:ty) => {
        impl<'b, T: 'b + ?Sized + IdOrdItem> IdOrdItem for $type {
            type Key<'a>
                = T::Key<'a>
            where
                Self: 'a;

            fn key(&self) -> Self::Key<'_> {
                (**self).key()
            }

            fn upcast_key<'short, 'long: 'short>(
                long: Self::Key<'long>,
            ) -> Self::Key<'short>
            where
                Self: 'long,
            {
                T::upcast_key(long)
            }
        }
    };
}

impl_for_ref!(&'b T);
impl_for_ref!(&'b mut T);

macro_rules! impl_for_box {
    ($type:ty) => {
        impl<T: ?Sized + IdOrdItem> IdOrdItem for $type {
            type Key<'a>
                = T::Key<'a>
            where
                Self: 'a;

            fn key(&self) -> Self::Key<'_> {
                (**self).key()
            }

            fn upcast_key<'short, 'long: 'short>(
                long: Self::Key<'long>,
            ) -> Self::Key<'short> {
                T::upcast_key(long)
            }
        }
    };
}

impl_for_box!(Box<T>);
impl_for_box!(Rc<T>);
impl_for_box!(Arc<T>);

mod sealed {
    pub trait Sealed<'a> {}
}

/// A trait for mutable access to items in an [`IdOrdMap`].
///
/// Mutable access to items within an [`IdOrdMap`] requires that the key type
/// implement [`Hash`], so that hash equality is checked on drop. For more
/// information, see [`RefMut`].
///
/// This is a sealed trait that's automatically implemented whenever `T::Key`
/// implements [`Hash`].
///
/// [`IdOrdMap`]: crate::IdOrdMap
/// [`RefMut`]: crate::id_ord_map::RefMut
pub trait IdOrdItemMut<'a>:
    IdOrdItem<Key<'a>: Hash> + sealed::Sealed<'a> + 'a
{
}

impl<'a, T> IdOrdItemMut<'a> for T where T: 'a + IdOrdItem<Key<'a>: Hash> {}
impl<'a, T> sealed::Sealed<'a> for T where T: 'a + IdOrdItem<Key<'a>: Hash> {}
