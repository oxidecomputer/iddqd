// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Trait definitions for `TriHashMap`.

use std::hash::Hash;

pub trait TriHashMapEntry: Clone {
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
