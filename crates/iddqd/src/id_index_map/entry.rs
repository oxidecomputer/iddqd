use super::{IdHashItem, IdIndexMap, RefMut};
use crate::{
    DefaultHashBuilder,
    support::{
        alloc::{Allocator, Global},
        borrow::DormantMutRef,
        map_hash::MapHash,
    },
};
use core::{fmt, hash::BuildHasher};

/// An implementation of the Entry API for [`IdIndexMap`].
pub enum Entry<'a, T: IdHashItem, S = DefaultHashBuilder, A: Allocator = Global>
{
    /// A vacant entry.
    Vacant(VacantEntry<'a, T, S, A>),
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, T, S, A>),
}

impl<'a, T: IdHashItem, S, A: Allocator> fmt::Debug for Entry<'a, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Vacant(entry) => {
                f.debug_tuple("Vacant").field(entry).finish()
            }
            Entry::Occupied(entry) => {
                f.debug_tuple("Occupied").field(entry).finish()
            }
        }
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    Entry<'a, T, S, A>
{
    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`IdIndexMap::entry`].
    #[inline]
    pub fn or_insert(self, default: T) -> RefMut<'a, T, S> {
        // TODO: Implement
        todo!()
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`IdIndexMap::entry`].
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> T>(
        self,
        default: F,
    ) -> RefMut<'a, T, S> {
        // TODO: Implement
        todo!()
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(RefMut<'_, T, S>),
    {
        // TODO: Implement
        todo!()
    }

    /// Returns the index of this entry in the map.
    ///
    /// For vacant entries, this returns the index where the entry would be inserted.
    pub fn index(&self) -> usize {
        // TODO: Implement
        todo!()
    }
}

/// A vacant entry.
pub struct VacantEntry<
    'a,
    T: IdHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, IdIndexMap<T, S, A>>,
    hash: MapHash<S>,
}

impl<'a, T: IdHashItem, S, A: Allocator> fmt::Debug
    for VacantEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry")
            .field("hash", &self.hash)
            .finish_non_exhaustive()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    VacantEntry<'a, T, S, A>
{
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, IdIndexMap<T, S, A>>,
        hash: MapHash<S>,
    ) -> Self {
        VacantEntry { map, hash }
    }

    /// Sets the entry to a new value, returning a mutable reference to the
    /// value.
    pub fn insert(self, value: T) -> RefMut<'a, T, S> {
        // TODO: Implement
        todo!()
    }

    /// Sets the value of the entry, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(mut self, value: T) -> OccupiedEntry<'a, T, S, A> {
        // TODO: Implement
        todo!()
    }

    /// Returns the index where this entry would be inserted.
    pub fn index(&self) -> usize {
        // TODO: Implement
        todo!()
    }
}

/// A view into an occupied entry in an [`IdIndexMap`]. Part of the [`Entry`]
/// enum.
pub struct OccupiedEntry<
    'a,
    T: IdHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, IdIndexMap<T, S, A>>,
    // index is a valid index into the map's internal ordered storage.
    index: usize,
}

impl<'a, T: IdHashItem, S, A: Allocator> fmt::Debug
    for OccupiedEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    OccupiedEntry<'a, T, S, A>
{
    /// # Safety
    ///
    /// After self is created, the original reference created by
    /// `DormantMutRef::new` must not be used.
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, IdIndexMap<T, S, A>>,
        index: usize,
    ) -> Self {
        OccupiedEntry { map, index }
    }

    /// Gets a reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_ref`](Self::into_ref).
    pub fn get(&self) -> &T {
        // TODO: Implement
        todo!()
    }

    /// Gets a mutable reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_mut`](Self::into_mut).
    pub fn get_mut(&mut self) -> RefMut<'_, T, S> {
        // TODO: Implement
        todo!()
    }

    /// Converts self into a reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get`](Self::get).
    pub fn into_ref(self) -> &'a T {
        // TODO: Implement
        todo!()
    }

    /// Converts self into a mutable reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get_mut`](Self::get_mut).
    pub fn into_mut(self) -> RefMut<'a, T, S> {
        // TODO: Implement
        todo!()
    }

    /// Sets the entry to a new value, returning the old value.
    ///
    /// # Panics
    ///
    /// Panics if `value.key()` is different from the key of the entry.
    pub fn insert(&mut self, value: T) -> T {
        // TODO: Implement
        todo!()
    }

    /// Takes ownership of the value from the map.
    pub fn remove(mut self) -> T {
        // TODO: Implement
        todo!()
    }

    /// Takes ownership of the value from the map, shifting all elements after it.
    pub fn shift_remove(self) -> T {
        // TODO: Implement
        todo!()
    }

    /// Takes ownership of the value from the map, swapping it with the last element.
    pub fn swap_remove(self) -> T {
        // TODO: Implement
        todo!()
    }

    /// Returns the index of this entry in the map.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Moves this entry to a new index.
    pub fn move_to(&mut self, new_index: usize) {
        // TODO: Implement
        todo!()
    }

    /// Swaps this entry with another entry at the given index.
    pub fn swap_with(&mut self, other_index: usize) {
        // TODO: Implement
        todo!()
    }
}
