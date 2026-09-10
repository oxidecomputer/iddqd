use super::{IdOrdItem, RefMut, tables::IdOrdMapTables};
use crate::support::{
    alloc::Global,
    btree_table,
    item_set::{ConsumingItemSet, ItemSet, ItemSlotsPtr},
};
use core::iter::FusedIterator;

/// An iterator over the elements of an [`IdOrdMap`] by shared reference.
///
/// Created by [`IdOrdMap::iter`], and ordered by keys.
///
/// [`IdOrdMap`]: crate::IdOrdMap
/// [`IdOrdMap::iter`]: crate::IdOrdMap::iter
#[derive(Clone, Debug)]
pub struct Iter<'a, T: IdOrdItem> {
    items: &'a ItemSet<T, Global>,
    iter: btree_table::Iter<'a>,
}

impl<'a, T: IdOrdItem> Iter<'a, T> {
    pub(super) fn new(
        items: &'a ItemSet<T, Global>,
        tables: &'a IdOrdMapTables,
    ) -> Self {
        Self { items, iter: tables.key_to_item.iter() }
    }
}

impl<'a, T: IdOrdItem> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;
        Some(&self.items[index])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T: IdOrdItem> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

// btree_set::Iter is a FusedIterator, so Iter is as well.
impl<T: IdOrdItem> FusedIterator for Iter<'_, T> {}

/// An iterator over the elements of a [`IdOrdMap`] by mutable reference.
///
/// This iterator returns [`RefMut`] instances.
///
/// Created by [`IdOrdMap::iter_mut`], and ordered by keys.
///
/// [`IdOrdMap`]: crate::IdOrdMap
/// [`IdOrdMap::iter_mut`]: crate::IdOrdMap::iter_mut
#[derive(Debug)]
pub struct IterMut<'a, T: IdOrdItem> {
    items: ItemSlotsPtr<'a, T>,
    tables: &'a IdOrdMapTables,
    iter: btree_table::Iter<'a>,
}

impl<'a, T: IdOrdItem> IterMut<'a, T> {
    pub(super) fn new(
        items: &'a mut ItemSet<T, Global>,
        tables: &'a IdOrdMapTables,
    ) -> Self {
        Self {
            items: ItemSlotsPtr::new(items.slots_mut()),
            tables,
            iter: tables.key_to_item.iter(),
        }
    }
}

impl<'a, T: IdOrdItem> Iterator for IterMut<'a, T> {
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;

        // SAFETY: `get_mut` requires that we pass each `index` at most once
        // for the lifetime of `self.items`. We do, because:
        //
        // * `self.iter` visits each entry of the B-tree once, and
        // * no two entries hold the same index (the no-duplicate invariant in
        //   the `btree_table` module docs).
        //
        // The user's `Ord` is not called at any point, since iteration is
        // structural.
        let item: &'a mut T = unsafe { self.items.get_mut(index) };

        let hash = self.tables.make_hash(item);

        Some(RefMut::new(self.tables.state().clone(), hash, item))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T: IdOrdItem> ExactSizeIterator for IterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, T: IdOrdItem> FusedIterator for IterMut<'a, T> {}

/// An iterator over the elements of a [`IdOrdMap`] by ownership.
///
/// Created by [`IdOrdMap::into_iter`], and ordered by keys.
///
/// [`IdOrdMap`]: crate::IdOrdMap
/// [`IdOrdMap::into_iter`]: crate::IdOrdMap::into_iter
#[derive(Debug)]
pub struct IntoIter<T: IdOrdItem> {
    items: ConsumingItemSet<T, Global>,
    iter: btree_table::IntoIter,
}

impl<T: IdOrdItem> IntoIter<T> {
    pub(super) fn new(
        items: ItemSet<T, Global>,
        tables: IdOrdMapTables,
    ) -> Self {
        Self {
            items: items.into_consuming(),
            iter: tables.key_to_item.into_iter(),
        }
    }
}

impl<T: IdOrdItem> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;
        // We own `self.items` and the B-tree's indexes are never revisited, so
        // we can take directly from the consuming view.
        let next = self
            .items
            .take(index)
            .unwrap_or_else(|| panic!("index {index} not found in items"));
        Some(next)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T: IdOrdItem> ExactSizeIterator for IntoIter<T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

// btree_map::IntoIter is a FusedIterator, so IntoIter is as well.
impl<T: IdOrdItem> FusedIterator for IntoIter<T> {}
