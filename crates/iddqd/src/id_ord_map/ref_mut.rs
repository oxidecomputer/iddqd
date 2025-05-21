// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::IdOrdItem;
use crate::support::map_hash::MapHash;
use std::{
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
};

/// A mutable reference to an [`IdOrdMap`] entry.
///
/// This is a wrapper around a `&mut T` that panics when dropped, if the
/// borrowed value's key has changed since the wrapper was created.
///
/// # Change detection
///
/// `RefMut` uses an owned form of the key to compare equality with. For this
/// purpose, `RefMut` requires that `IdOrdItemMut` be implemented.
///
/// [`IdOrdMap`]: crate::IdOrdMap
pub struct RefMut<'a, T: IdOrdItem>
where
    for<'k> T::Key<'k>: Hash,
{
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: IdOrdItem> RefMut<'a, T>
where
    for<'k> T::Key<'k>: Hash,
{
    pub(super) fn new(hash: MapHash, borrowed: &'a mut T) -> Self {
        let inner = RefMutInner { hash, borrowed };
        Self { inner: Some(inner) }
    }

    /// Converts this `RefMut` into a `&'a T`.
    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<T: IdOrdItem> Drop for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<T: IdOrdItem> Deref for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<T: IdOrdItem> DerefMut for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

impl<T: IdOrdItem + fmt::Debug> fmt::Debug for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            Some(ref inner) => inner.fmt(f),
            None => {
                f.debug_struct("RefMut").field("borrowed", &"missing").finish()
            }
        }
    }
}

struct RefMutInner<'a, T: IdOrdItem> {
    hash: MapHash,
    borrowed: &'a mut T,
}

impl<'a, T: IdOrdItem> RefMutInner<'a, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn into_ref(self) -> &'a T {
        if !self.hash.is_same_hash(self.borrowed.key()) {
            panic!("key changed during RefMut borrow");
        }

        self.borrowed
    }
}

impl<T: IdOrdItem + fmt::Debug> fmt::Debug for RefMutInner<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrowed.fmt(f)
    }
}
