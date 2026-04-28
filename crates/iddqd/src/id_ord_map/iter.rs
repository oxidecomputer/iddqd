use super::{IdOrdItem, RefMut, tables::IdOrdMapTables};
use crate::support::{
    alloc::Global,
    borrow::DormantMutRef,
    btree_table,
    item_set::{ConsumingItemSet, ItemSet, SlabEntry},
};
use core::{hash::Hash, iter::FusedIterator, marker::PhantomData};

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

/// A raw pointer into an `ItemSet`'s slot buffer, with the same thread-safety
/// properties as an `&'a mut ItemSet<T, Global>`.
///
/// We use a raw pointer rather than lifetime extension as done by the hash map
/// iterators to avoid reborrow invalidation under Stacked Borrows. Due to the
/// way Vec::index_mut works, each iteration reborrowing `&mut self.items` would
/// invalidate previously yielded `&mut T` children.
struct ItemSetPtr<'a, T: IdOrdItem> {
    ptr: *mut SlabEntry<T>,
    // Number of slots in the backing buffer at construction time.
    slot_count: usize,
    // Borrow the ItemSet for `'a` so the raw pointer stays live, and so that
    // variance and drop-check work the same as `&'a mut ItemSet<T, Global>`.
    _marker: PhantomData<&'a mut ItemSet<T, Global>>,
}

impl<T: IdOrdItem> core::fmt::Debug for ItemSetPtr<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ItemSetPtr")
            .field("ptr", &self.ptr)
            .field("slot_count", &self.slot_count)
            .finish()
    }
}

// SAFETY: `ItemSetPtr<'a, T>` has the same thread-safety semantics as `&'a mut
// ItemSet<T, Global>`, which is `Send`/`Sync` iff `ItemSet<T, Global>` is
// either of those, respectively. This reduces to `T: Send` / `T: Sync`, since
// the global allocator `Global` is always `Send` + `Sync`.
unsafe impl<'a, T: IdOrdItem + Send> Send for ItemSetPtr<'a, T> {}
// SAFETY: see the `Send` impl above.
unsafe impl<'a, T: IdOrdItem + Sync> Sync for ItemSetPtr<'a, T> {}

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
    items: ItemSetPtr<'a, T>,
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
        let slot_count = items.slot_count();
        let ptr = items.as_mut_ptr();
        Self {
            items: ItemSetPtr { ptr, slot_count, _marker: PhantomData },
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
        let raw_index = index.as_u32() as usize;

        // This is a belt-and-suspenders bounds check. As of 2026-04-28, we've
        // carefully analyzed all the code paths (including for panic safety) to
        // ensure that indexes stored in the B-tree are always in bounds. But a
        // future change might inadvertently break things. Handle this kind of
        // programmer error as a panic rather than UB.
        assert!(
            raw_index < self.items.slot_count,
            "btree index {raw_index} out of bounds for slot count {}",
            self.items.slot_count,
        );

        // SAFETY: We need to show:
        //
        // * `self.items.ptr.add(raw_index)` points at valid memory.
        // * There are no overlapping mutable borrows of the same memory.
        //
        // This is shown by the following observations:
        //
        // * We construct `ItemSetPtr` by mutably borrowing the item set,
        //   which means that while this iterator is alive, no other code
        //   can access the item set.
        // * The bounds check above shows that `raw_index` is in bounds.
        // * The B-tree only stores indexes that currently point at `Some`
        //   slots in the backing `ItemSet`, so the slot is initialized.
        //   (Again, as of 2026-04-28 we've verified this invariant, but
        //   a future change might break things, so we use `expect` and not
        //   `unwrap_unchecked`.)
        // * The B-tree is a set, so each call to `self.iter.next()` yields a
        //   distinct `index`. This means that the handed-out `&mut T`s
        //   never point to the same memory.
        let item: &'a mut T = unsafe {
            (*self.items.ptr.add(raw_index))
                .as_mut()
                .expect("btree index points at an Occupied slot in ItemSet")
        };

        let (hash, dormant) = {
            let (item, dormant) = DormantMutRef::new(item);
            let hash = self.tables.make_hash(item);
            (hash, dormant)
        };

        // SAFETY: item is dropped above, and self is no longer used
        // after this point.
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
        // We own `self.items` and the btree's indexes are never revisited,
        // so take directly from the consuming view (O(1), no free-list
        // allocation) rather than `ItemSet::remove`, which would push to
        // the free list per call.
        let next = self
            .items
            .take(index)
            .unwrap_or_else(|| panic!("index {index} not found in items"));
        Some(next)
    }
}
