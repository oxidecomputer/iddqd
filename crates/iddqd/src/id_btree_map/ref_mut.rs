// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::IdOrdItemMut;
use derive_where::derive_where;
use std::{
    fmt,
    ops::{Deref, DerefMut},
};

/// A mutable reference to an [`IdBTreeMap`] entry.
///
/// This is a wrapper around a `&mut T` that panics when dropped, if the
/// borrowed value's key has changed since the wrapper was created.
///
/// # Change detection
///
/// `RefMut` uses an owned form of the key to compare equality with. For this
/// purpose, `RefMut` requires that `IdOrdItemMut` be implemented.
///
/// [`IdBTreeMap`]: crate::IdBTreeMap
#[derive_where(Debug; T: fmt::Debug, T::OwnedKey: fmt::Debug)]
pub struct RefMut<'a, T: IdOrdItemMut> {
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: IdOrdItemMut> RefMut<'a, T> {
    pub(super) fn new(borrowed: &'a mut T) -> Self {
        let key = borrowed.owned_key();
        let inner = RefMutInner { borrowed, key };
        Self { inner: Some(inner) }
    }

    /// Converts this `RefMut` into a `&'a T`.
    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<T: IdOrdItemMut> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<T: IdOrdItemMut> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<T: IdOrdItemMut> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

#[derive_where(Debug; T: fmt::Debug, T::OwnedKey: fmt::Debug)]
struct RefMutInner<'a, T: IdOrdItemMut> {
    key: T::OwnedKey,
    borrowed: &'a mut T,
}

impl<'a, T: IdOrdItemMut> RefMutInner<'a, T> {
    fn into_ref(self) -> &'a T {
        let new_key = self.borrowed.owned_key();
        if new_key != self.key {
            panic!("key changed during RefMut borrow");
        }

        self.borrowed
    }
}
