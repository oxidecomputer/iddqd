use super::{IdOrdItem, RefMut, tables::IdOrdMapTables};
use crate::support::{
    alloc::Global,
    borrow::DormantMutRef,
    btree_table,
    item_set::{ConsumingItemSet, ItemSet, ItemSlotsPtr},
};
use core::{hash::Hash, iter::FusedIterator};

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
pub struct IterMut<'a, T: IdOrdItem>
where
    T::Key<'a>: Hash,
{
    items: ItemSlotsPtr<'a, T>,
    tables: &'a IdOrdMapTables,
    iter: btree_table::Iter<'a>,
}

impl<'a, T: IdOrdItem> IterMut<'a, T>
where
    T::Key<'a>: Hash,
{
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

impl<'a, T: IdOrdItem + 'a> Iterator for IterMut<'a, T>
where
    T::Key<'a>: Hash,
{
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;

        // SAFETY: The B-tree is a set, so each call to `self.iter.next()`
        // yields a distinct `index`. Therefore the `&mut T` references that
        // `get_mut` hands out across iterations never alias.
        let item: &'a mut T = unsafe { self.items.get_mut(index) };

        let (hash, dormant) = {
            let (item, dormant) = DormantMutRef::new(item);
            let hash = self.tables.make_hash(item);
            (hash, dormant)
        };

        // SAFETY: The `&mut T` that `DormantMutRef::new` produced inside
        // the block above (and used for hashing) was dropped when the
        // block closed, so the dormant ref is now the unique borrow of
        // the slot. The `self.tables.state()` access below touches a
        // different allocation and does not alias.
        let item = unsafe { dormant.awaken() };

        Some(RefMut::new(self.tables.state().clone(), hash, item))
    }
}

impl<'a, T: IdOrdItem + 'a> ExactSizeIterator for IterMut<'a, T>
where
    T::Key<'a>: Hash,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, T: IdOrdItem + 'a> FusedIterator for IterMut<'a, T> where
    T::Key<'a>: Hash
{
}

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
}
