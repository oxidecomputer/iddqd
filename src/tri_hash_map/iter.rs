// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::iter::FusedIterator;

/// An iterator over the elements of a [`TriHashMap`].
///
/// Created by [`TriHashMap::iter`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter`]: crate::TriHashMap::iter
#[derive(Clone, Debug, Default)]
pub struct Iter<'a, T> {
    inner: std::slice::Iter<'a, T>,
}

impl<'a, T> Iter<'a, T> {
    pub(crate) fn new(entries: &'a [T]) -> Self {
        Self { inner: entries.iter() }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T> FusedIterator for Iter<'_, T> {}
