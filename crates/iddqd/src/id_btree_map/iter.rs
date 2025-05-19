// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{tables::IdBTreeMapTables, IdOrdItem, IdOrdItemMut, RefMut};
use crate::support::{btree_table, entry_set::EntrySet};
use std::iter::FusedIterator;

/// An iterator over the elements of an [`IdBTreeMap`] by shared reference.
///
/// Created by [`IdBTreeMap::iter`], and ordered by keys.
///
/// [`IdBTreeMap`]: crate::IdBTreeMap
/// [`IdBTreeMap::iter`]: crate::IdBTreeMap::iter
#[derive(Clone, Debug)]
pub struct Iter<'a, T: IdOrdItem> {
    entries: &'a EntrySet<T>,
    iter: btree_table::Iter<'a>,
}

impl<'a, T: IdOrdItem> Iter<'a, T> {
    pub(super) fn new(
        entries: &'a EntrySet<T>,
        tables: &'a IdBTreeMapTables,
    ) -> Self {
        Self { entries, iter: tables.key_to_entry.iter() }
    }
}

impl<'a, T: IdOrdItem> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;
        Some(&self.entries[index])
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

/// An iterator over the elements of a [`IdBTreeMap`] by mutable reference.
///
/// This iterator returns [`RefMut`] instances.
///
/// Created by [`IdBTreeMap::iter_mut`], and ordered by keys.
///
/// [`IdBTreeMap`]: crate::IdBTreeMap
/// [`IdBTreeMap::iter_mut`]: crate::IdBTreeMap::iter_mut
#[derive(Debug)]
pub struct IterMut<'a, T: IdOrdItemMut> {
    entries: &'a mut EntrySet<T>,
    iter: btree_table::Iter<'a>,
}

impl<'a, T: IdOrdItemMut> IterMut<'a, T> {
    pub(super) fn new(
        entries: &'a mut EntrySet<T>,
        tables: &'a IdBTreeMapTables,
    ) -> Self {
        Self { entries, iter: tables.key_to_entry.iter() }
    }
}

impl<'a, T: IdOrdItemMut + 'a> Iterator for IterMut<'a, T> {
    type Item = RefMut<'a, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;
        let entry = &mut self.entries[index];

        // SAFETY: This lifetime extension from self to 'a is safe based on two
        // things:
        //
        // 1. We never repeat indexes, i.e. for an index i, once we've handed
        //    out an entry at i, creating `&mut T`, we'll never get the index i
        //    again. (This is guaranteed from the set-based nature of the
        //    iterator.) This means that we don't ever create a mutable alias to
        //    the same memory.
        //
        //    In particular, unlike all the other places we look up data from a
        //    btree table, we don't pass a lookup function into
        //    self.iter.next(). If we did, then it is possible the lookup
        //    function would have been called with an old index i. But we don't
        //    need to do that.
        //
        // 2. All mutable references to data within self.entries are derived
        //    from self.entries. So, the rule described at [1] is upheld:
        //
        //    > When creating a mutable reference, then while this reference
        //    > exists, the memory it points to must not get accessed (read or
        //    > written) through any other pointer or reference not derived from
        //    > this reference.
        //
        // [1]:
        //     https://doc.rust-lang.org/std/ptr/index.html#pointer-to-reference-conversion
        let entry = unsafe { std::mem::transmute::<&mut T, &'a mut T>(entry) };
        Some(RefMut::new(entry))
    }
}

impl<'a, T: IdOrdItemMut + 'a> ExactSizeIterator for IterMut<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

// hash_map::IterMut is a FusedIterator, so IterMut is as well.
impl<'a, T: IdOrdItemMut + 'a> FusedIterator for IterMut<'a, T> {}

/// An iterator over the elements of a [`IdBTreeMap`] by ownership.
///
/// Created by [`IdBTreeMap::into_iter`], and ordered by keys.
///
/// [`IdBTreeMap`]: crate::IdBTreeMap
/// [`IdBTreeMap::into_iter`]: crate::IdBTreeMap::into_iter
#[derive(Debug)]
pub struct IntoIter<T: IdOrdItem> {
    entries: EntrySet<T>,
    iter: btree_table::IntoIter,
}

impl<T: IdOrdItem> IntoIter<T> {
    pub(super) fn new(entries: EntrySet<T>, tables: IdBTreeMapTables) -> Self {
        Self { entries, iter: tables.key_to_entry.into_iter() }
    }
}

impl<T: IdOrdItem> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.iter.next()?;
        let next = self
            .entries
            .remove(index)
            .unwrap_or_else(|| panic!("index {index} not found in entries"));
        Some(next)
    }
}
