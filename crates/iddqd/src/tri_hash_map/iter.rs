use super::{RefMut, tables::TriHashMapTables};
use crate::{TriHashItem, support::item_set::ItemSet};
use hashbrown::hash_map;
use std::iter::FusedIterator;

/// An iterator over the elements of a [`TriHashMap`] by shared reference.
///
/// Created by [`TriHashMap::iter`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter`]: crate::TriHashMap::iter
#[derive(Clone, Debug, Default)]
pub struct Iter<'a, T: TriHashItem> {
    inner: hash_map::Values<'a, usize, T>,
}

impl<'a, T: TriHashItem> Iter<'a, T> {
    pub(crate) fn new(items: &'a ItemSet<T>) -> Self {
        Self { inner: items.values() }
    }
}

impl<'a, T: TriHashItem> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T: TriHashItem> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::Iter is a FusedIterator, so Iter is as well.
impl<T: TriHashItem> FusedIterator for Iter<'_, T> {}

/// An iterator over the elements of a [`TriHashMap`] by mutable reference.
///
/// This iterator returns [`RefMut`] instances.
///
/// Created by [`TriHashMap::iter_mut`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter_mut`]: crate::TriHashMap::iter_mut
#[derive(Debug)]
pub struct IterMut<'a, T: TriHashItem> {
    tables: &'a TriHashMapTables,
    inner: hash_map::ValuesMut<'a, usize, T>,
}

impl<'a, T: TriHashItem> IterMut<'a, T> {
    pub(super) fn new(
        tables: &'a TriHashMapTables,
        items: &'a mut ItemSet<T>,
    ) -> Self {
        Self { tables, inner: items.values_mut() }
    }
}

impl<'a, T: TriHashItem> Iterator for IterMut<'a, T> {
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next()?;
        let hashes = self.tables.make_hashes(next);
        Some(RefMut::new(hashes, next))
    }
}

impl<T: TriHashItem> ExactSizeIterator for IterMut<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<T: TriHashItem> FusedIterator for IterMut<'_, T> {}

/// An iterator over the elements of a [`TriHashMap`] by ownership.
///
/// Created by [`TriHashMap::into_iter`].
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::into_iter`]: crate::TriHashMap::into_iter
#[derive(Debug)]
pub struct IntoIter<T: TriHashItem> {
    inner: hash_map::IntoValues<usize, T>,
}

impl<T: TriHashItem> IntoIter<T> {
    pub(crate) fn new(items: ItemSet<T>) -> Self {
        Self { inner: items.into_values() }
    }
}

impl<T: TriHashItem> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T: TriHashItem> ExactSizeIterator for IntoIter<T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<T: TriHashItem> FusedIterator for IntoIter<T> {}
