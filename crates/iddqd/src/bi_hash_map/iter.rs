use super::{RefMut, tables::BiHashMapTables};
use crate::{BiHashItem, support::item_set::ItemSet};
use hashbrown::hash_map;
use std::iter::FusedIterator;

/// An iterator over the elements of a [`BiHashMap`] by shared reference.
///
/// Created by [`BiHashMap::iter`].
///
/// [`BiHashMap`]: crate::BiHashMap
/// [`BiHashMap::iter`]: crate::BiHashMap::iter
#[derive(Clone, Debug, Default)]
pub struct Iter<'a, T: BiHashItem> {
    inner: hash_map::Values<'a, usize, T>,
}

impl<'a, T: BiHashItem> Iter<'a, T> {
    pub(crate) fn new(items: &'a ItemSet<T>) -> Self {
        Self { inner: items.values() }
    }
}

impl<'a, T: BiHashItem> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T: BiHashItem> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::Iter is a FusedIterator, so Iter is as well.
impl<T: BiHashItem> FusedIterator for Iter<'_, T> {}

/// An iterator over the elements of a [`BiHashMap`] by mutable reference.
///
/// This iterator returns [`RefMut`] instances.
///
/// Created by [`BiHashMap::iter_mut`].
///
/// [`BiHashMap`]: crate::BiHashMap
/// [`BiHashMap::iter_mut`]: crate::BiHashMap::iter_mut
#[derive(Debug)]
pub struct IterMut<'a, T: BiHashItem> {
    tables: &'a BiHashMapTables,
    inner: hash_map::ValuesMut<'a, usize, T>,
}

impl<'a, T: BiHashItem> IterMut<'a, T> {
    pub(super) fn new(
        tables: &'a BiHashMapTables,
        items: &'a mut ItemSet<T>,
    ) -> Self {
        Self { tables, inner: items.values_mut() }
    }
}

impl<'a, T: BiHashItem> Iterator for IterMut<'a, T> {
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next()?;
        let hashes = self.tables.make_hashes::<T>(&next.key1(), &next.key2());
        Some(RefMut::new(hashes, next))
    }
}

impl<T: BiHashItem> ExactSizeIterator for IterMut<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<T: BiHashItem> FusedIterator for IterMut<'_, T> {}

/// An iterator over the elements of a [`BiHashMap`] by ownership.
///
/// Created by [`BiHashMap::into_iter`].
///
/// [`BiHashMap`]: crate::BiHashMap
/// [`BiHashMap::into_iter`]: crate::BiHashMap::into_iter
#[derive(Debug)]
pub struct IntoIter<T: BiHashItem> {
    inner: hash_map::IntoValues<usize, T>,
}

impl<T: BiHashItem> IntoIter<T> {
    pub(crate) fn new(items: ItemSet<T>) -> Self {
        Self { inner: items.into_values() }
    }
}

impl<T: BiHashItem> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<T: BiHashItem> ExactSizeIterator for IntoIter<T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<T: BiHashItem> FusedIterator for IntoIter<T> {}
