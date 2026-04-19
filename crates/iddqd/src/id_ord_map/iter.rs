use super::{IdOrdItem, RefMut, tables::IdOrdMapTables};
use crate::support::{
    alloc::Global,
    borrow::DormantMutRef,
    btree_table,
    item_set::{ConsumingItemSet, ItemSet},
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

// A raw pointer into an `ItemSet`'s slot buffer with the same
// thread-safety posture as an `&'a mut ItemSet<T, Global>`.
//
// We use a raw pointer rather than a reference inside `IterMut` to
// avoid reborrow invalidation under Tree Borrows — each iteration
// reborrowing `&mut self.items` would invalidate previously yielded
// `&mut T` children. Wrapping the raw pointer in a dedicated struct
// (instead of a bare field + manual `Send`/`Sync` on `IterMut`) lets
// the compiler auto-derive `IterMut`'s auto traits from the
// combination of *all* its fields, so if a future `IdOrdMapTables` or
// `btree_table::Iter` field becomes non-`Send` / non-`Sync`,
// `IterMut` follows automatically.
struct ItemSetPtr<'a, T: IdOrdItem> {
    ptr: *mut Option<T>,
    // Borrow the ItemSet for `'a` so the raw pointer stays live, and
    // so variance / drop-check mirror `&'a mut ItemSet<T, Global>`.
    _marker: PhantomData<&'a mut ItemSet<T, Global>>,
}

impl<T: IdOrdItem> core::fmt::Debug for ItemSetPtr<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ItemSetPtr").field("ptr", &self.ptr).finish()
    }
}

// SAFETY: `ItemSetPtr<'a, T>` has the same thread-safety semantics as
// `&'a mut ItemSet<T, Global>`, which is `Send`/`Sync` iff
// `ItemSet<T, Global>` is, which reduces to `T: Send` / `T: Sync`
// (since `Global: Send + Sync` unconditionally).
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
        Self {
            items: ItemSetPtr { ptr: items.as_mut_ptr(), _marker: PhantomData },
            tables,
            iter: tables.key_to_item.iter(),
        }
    }
}

// `Send` / `Sync` are auto-derived: `ItemSetPtr<'a, T>` contributes the
// `T: Send`/`Sync` bound that `&'a mut ItemSet<T, Global>` would,
// `&'a IdOrdMapTables` contributes `IdOrdMapTables: Sync`, and
// `btree_table::Iter<'a>` contributes whatever its fields require.
// If any of those ever loses `Send`/`Sync`, `IterMut` follows without
// a silent manual impl masking the change.

impl<'a, T: IdOrdItem + 'a> Iterator for IterMut<'a, T>
where
    T::Key<'a>: Hash,
{
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;

        // SAFETY: the btree only stores indexes that currently point at
        // `Some` slots in the backing `ItemSet` (upheld by every
        // btree-mutating call site in `id_ord_map`), so
        // `items.ptr.add(index)` is in-bounds and the slot is
        // initialized. The btree is a set, so each call to
        // `self.iter.next()` yields a distinct `index`: the `&mut T`s
        // handed out across iterations target disjoint memory and never
        // alias. Since we never reborrow `&mut ItemSet` between
        // iterations, no ancestor reborrow invalidates
        // previously-yielded references.
        let item: &'a mut T = unsafe {
            (*self.items.ptr.add(index))
                .as_mut()
                .expect("btree index points at a Some slot in ItemSet")
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
