// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Trait definitions for `TriHashMap`.

use std::{hash::Hash, rc::Rc, sync::Arc};

pub trait TriHashMapEntry {
    type K1<'a>: Eq + Hash
    where
        Self: 'a;
    type K2<'a>: Eq + Hash
    where
        Self: 'a;
    type K3<'a>: Eq + Hash
    where
        Self: 'a;

    fn key1(&self) -> Self::K1<'_>;
    fn key2(&self) -> Self::K2<'_>;
    fn key3(&self) -> Self::K3<'_>;

    fn upcast_key1<'short, 'long: 'short>(
        long: Self::K1<'long>,
    ) -> Self::K1<'short>;
    fn upcast_key2<'short, 'long: 'short>(
        long: Self::K2<'long>,
    ) -> Self::K2<'short>;
    fn upcast_key3<'short, 'long: 'short>(
        long: Self::K3<'long>,
    ) -> Self::K3<'short>;
}

macro_rules! impl_for_ref {
    ($type:ty) => {
        impl<'b, T: 'b + ?Sized + TriHashMapEntry> TriHashMapEntry for $type {
            type K1<'a>
                = T::K1<'a>
            where
                Self: 'a;
            type K2<'a>
                = T::K2<'a>
            where
                Self: 'a;
            type K3<'a>
                = T::K3<'a>
            where
                Self: 'a;

            fn key1(&self) -> Self::K1<'_> {
                (**self).key1()
            }

            fn key2(&self) -> Self::K2<'_> {
                (**self).key2()
            }

            fn key3(&self) -> Self::K3<'_> {
                (**self).key3()
            }

            fn upcast_key1<'short, 'long: 'short>(
                long: Self::K1<'long>,
            ) -> Self::K1<'short>
            where
                Self: 'long,
            {
                T::upcast_key1(long)
            }

            fn upcast_key2<'short, 'long: 'short>(
                long: Self::K2<'long>,
            ) -> Self::K2<'short>
            where
                Self: 'long,
            {
                T::upcast_key2(long)
            }

            fn upcast_key3<'short, 'long: 'short>(
                long: Self::K3<'long>,
            ) -> Self::K3<'short>
            where
                Self: 'long,
            {
                T::upcast_key3(long)
            }
        }
    };
}

impl_for_ref!(&'b T);
impl_for_ref!(&'b mut T);

macro_rules! impl_for_box {
    ($type:ty) => {
        impl<T: ?Sized + TriHashMapEntry> TriHashMapEntry for $type {
            type K1<'a>
                = T::K1<'a>
            where
                Self: 'a;

            type K2<'a>
                = T::K2<'a>
            where
                Self: 'a;

            type K3<'a>
                = T::K3<'a>
            where
                Self: 'a;

            fn key1(&self) -> Self::K1<'_> {
                (**self).key1()
            }

            fn key2(&self) -> Self::K2<'_> {
                (**self).key2()
            }

            fn key3(&self) -> Self::K3<'_> {
                (**self).key3()
            }

            fn upcast_key1<'short, 'long: 'short>(
                long: Self::K1<'long>,
            ) -> Self::K1<'short> {
                T::upcast_key1(long)
            }

            fn upcast_key2<'short, 'long: 'short>(
                long: Self::K2<'long>,
            ) -> Self::K2<'short> {
                T::upcast_key2(long)
            }

            fn upcast_key3<'short, 'long: 'short>(
                long: Self::K3<'long>,
            ) -> Self::K3<'short> {
                T::upcast_key3(long)
            }
        }
    };
}

impl_for_box!(Box<T>);
impl_for_box!(Rc<T>);
impl_for_box!(Arc<T>);
