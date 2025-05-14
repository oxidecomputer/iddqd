// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Trait definitions for `IdBTreeMap`.

use std::{rc::Rc, sync::Arc};

/// An entry in an [`IdBTreeMap`].
///
/// This trait is used to define the keys.
///
/// # Examples
///
/// TODO: Add an example here.
///
/// [`IdBTreeMap`]: crate::IdBTreeMap
pub trait IdBTreeMapEntry {
    /// The key type.
    type Key<'a>: Ord
    where
        Self: 'a;

    /// Retrieves the key.
    fn key(&self) -> Self::Key<'_>;

    /// Upcasts the key to a shorter lifetime, in effect asserting that the
    /// lifetime `'a` on [`IdBTreeMapEntry::Key`] is covariant.
    ///
    /// Typically implemented via the [`id_upcast`] macro.
    fn upcast_key<'short, 'long: 'short>(
        long: Self::Key<'long>,
    ) -> Self::Key<'short>;
}

/// Required to be implemented for [`IdBTreeMap::get_mut`] to be called.
///
/// The `get_mut` method returns a wrapper which ensures that the key doesn't
/// change during mutation. This trait is used to return an owned form of the
/// key for temporary storage.
///
/// [`IdBTreeMap::get_mut`]: crate::IdBTreeMap::get_mut
pub trait IdBTreeMapEntryMut: IdBTreeMapEntry {
    /// An owned key type corresponding to [`IdBTreeMapEntry::Key`].
    ///
    /// This can also be a digest, or some other kind of value which changes iff
    /// the key changes.
    type OwnedKey: Eq;

    /// Retrieves the key as an owned value.
    fn owned_key(&self) -> Self::OwnedKey;
}

macro_rules! impl_for_ref {
    ($type:ty) => {
        impl<'b, T: 'b + ?Sized + IdBTreeMapEntry> IdBTreeMapEntry for $type {
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

        impl<'b, T: 'b + ?Sized + IdBTreeMapEntryMut> IdBTreeMapEntryMut
            for $type
        {
            type OwnedKey = T::OwnedKey;

            fn owned_key(&self) -> Self::OwnedKey {
                (**self).owned_key()
            }
        }
    };
}

impl_for_ref!(&'b T);
impl_for_ref!(&'b mut T);

macro_rules! impl_for_box {
    ($type:ty) => {
        impl<T: ?Sized + IdBTreeMapEntry> IdBTreeMapEntry for $type {
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

        impl<T: ?Sized + IdBTreeMapEntryMut> IdBTreeMapEntryMut for $type {
            type OwnedKey = T::OwnedKey;

            fn owned_key(&self) -> Self::OwnedKey {
                (**self).owned_key()
            }
        }
    };
}

impl_for_box!(Box<T>);
impl_for_box!(Rc<T>);
impl_for_box!(Arc<T>);
