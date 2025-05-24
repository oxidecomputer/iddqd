use super::{RefMut, tables::TriHashMapTables};
use crate::{DefaultHashBuilder, TriHashItem, support::item_set::ItemSet};
use core::{hash::BuildHasher, iter::FusedIterator};
use hashbrown::hash_map;

/// An iterator over the elements of a [`TriHashMap`] by shared reference.
/// Created by [`TriHashMap::iter`].
///
/// Similar to [`HashMap`], the iteration order is arbitrary and not guaranteed
/// to be stable.
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter`]: crate::TriHashMap::iter
/// [`HashMap`]: std::collections::HashMap
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
/// Created by [`TriHashMap::iter_mut`].
///
/// This iterator returns [`RefMut`] instances.
///
/// Similar to [`HashMap`], the iteration order is arbitrary and not guaranteed
/// to be stable.
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::iter_mut`]: crate::TriHashMap::iter_mut
/// [`HashMap`]: std::collections::HashMap
#[derive(Debug)]
pub struct IterMut<
    'a,
    T: TriHashItem,
    S: Clone + BuildHasher = DefaultHashBuilder,
> {
    tables: &'a TriHashMapTables<S>,
    inner: hash_map::ValuesMut<'a, usize, T>,
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher> IterMut<'a, T, S> {
    pub(super) fn new(
        tables: &'a TriHashMapTables<S>,
        items: &'a mut ItemSet<T>,
    ) -> Self {
        Self { tables, inner: items.values_mut() }
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher> Iterator
    for IterMut<'a, T, S>
{
    type Item = RefMut<'a, T, S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next()?;
        let hashes = self.tables.make_hashes(next);
        Some(RefMut::new(hashes, next))
    }
}

impl<T: TriHashItem, S: Clone + BuildHasher> ExactSizeIterator
    for IterMut<'_, T, S>
{
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<T: TriHashItem, S: Clone + BuildHasher> FusedIterator
    for IterMut<'_, T, S>
{
}

/// An iterator over the elements of a [`TriHashMap`] by ownership. Created by
/// [`TriHashMap::into_iter`].
///
/// Similar to [`HashMap`], the iteration order is arbitrary and not guaranteed
/// to be stable.
///
/// [`TriHashMap`]: crate::TriHashMap
/// [`TriHashMap::into_iter`]: crate::TriHashMap::into_iter
/// [`HashMap`]: std::collections::HashMap
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
