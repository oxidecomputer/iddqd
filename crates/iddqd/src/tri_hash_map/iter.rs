// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{imp::TriHashMapTables, RefMut};
use crate::{support::entry_set::EntrySet, TriHashMapEntry};
use std::{collections::hash_map, iter::FusedIterator};

/// An iterator over the elements of a [`TriHashMap`] by shared reference.
///
/// Created by [`TriHashMap::iter`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter`]: crate::TriHashMap::iter
#[derive(Clone, Debug, Default)]
pub struct Iter<'a, T: TriHashMapEntry> {
    inner: hash_map::Values<'a, usize, T>,
}

impl<'a, T: TriHashMapEntry> Iter<'a, T> {
    pub(crate) fn new(entries: &'a EntrySet<T>) -> Self {
        Self { inner: entries.values() }
    }
}

impl<'a, T: TriHashMapEntry> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T: TriHashMapEntry> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::Iter is a FusedIterator, so Iter is as well.
impl<T: TriHashMapEntry> FusedIterator for Iter<'_, T> {}

/// An iterator over the elements of a [`TriHashMap`] by mutable reference.
///
/// This iterator returns [`RefMut`] instances.
///
/// Created by [`TriHashMap::iter_mut`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter_mut`]: crate::TriHashMap::iter_mut
#[derive(Debug)]
pub struct IterMut<'a, T: TriHashMapEntry> {
    tables: &'a TriHashMapTables,
    inner: hash_map::ValuesMut<'a, usize, T>,
}

impl<'a, T: TriHashMapEntry> IterMut<'a, T> {
    pub(super) fn new(
        tables: &'a TriHashMapTables,
        entries: &'a mut EntrySet<T>,
    ) -> Self {
        Self { tables, inner: entries.values_mut() }
    }
}

impl<'a, T: TriHashMapEntry> Iterator for IterMut<'a, T> {
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next()?;
        let hashes = self.tables.make_hashes(next);
        Some(RefMut::new(hashes, next))
    }
}

impl<'a, T: TriHashMapEntry> ExactSizeIterator for IterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<'a, T: TriHashMapEntry> FusedIterator for IterMut<'a, T> {}
