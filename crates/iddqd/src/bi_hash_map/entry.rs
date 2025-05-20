// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{BiHashItem, BiHashMap, RefMut};
use crate::support::{borrow::DormantMutRef, hash_table::MapHash};
use debug_ignore::DebugIgnore;
use derive_where::derive_where;

/// An implementation of the Entry API for [`IdBTreeMap`].
#[derive_where(Debug)]
pub enum Entry<'a, T: BiHashItem> {
    /// A vacant entry.
    Vacant(VacantEntry<'a, T>),
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, T>),
}

impl<'a, T: BiHashItem> Entry<'a, T> {
    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`IdHashMap::entry`].
    #[inline]
    pub fn or_insert(self, default: T) -> RefMut<'a, T> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`IdHashMap::entry`].
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> T>(self, default: F) -> RefMut<'a, T> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(RefMut<'_, T>),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    /// Sets the value of the entry, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(self, value: T) -> OccupiedEntry<'a, T> {
        match self {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            Entry::Vacant(entry) => entry.insert_entry(value),
        }
    }
}

/// A vacant entry.
#[derive_where(Debug)]
pub struct VacantEntry<'a, T: BiHashItem> {
    map: DebugIgnore<DormantMutRef<'a, BiHashMap<T>>>,
    hash: EntryHash,
}

#[derive(Debug)]
pub(super) enum EntryHash {
    Key1(MapHash),
    Key2(MapHash),
}

impl EntryHash {
    #[inline]
    pub fn is_same_hash<T: BiHashItem>(&self, value: &T) -> bool {
        match self {
            EntryHash::Key1(h) => h.is_same_hash(value.key1()),
            EntryHash::Key2(h) => h.is_same_hash(value.key2()),
        }
    }
}

impl<'a, T: BiHashItem> VacantEntry<'a, T> {
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T>>,
        hash: EntryHash,
    ) -> Self {
        VacantEntry { map: map.into(), hash }
    }

    /// Sets the entry to a new value, returning a mutable reference to the
    /// value.
    pub fn insert(self, value: T) -> RefMut<'a, T> {
        if !self.hash.is_same_hash(&value) {
            panic!("key hashes do not match");
        }

        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.0.awaken() };
        let Ok(index) = map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        map.get_by_index_mut(index).expect("index is known to be valid")
    }

    /// Sets the value of the entry, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(mut self, value: T) -> OccupiedEntry<'a, T> {
        if !self.hash.is_same_hash(&value) {
            panic!("key hashes do not match");
        }

        let index = {
            // SAFETY: The safety assumption behind `Self::new` guarantees that the
            // original reference to the map is not used at this point.
            let map = unsafe { self.map.0.reborrow() };
            let Ok(index) = map.insert_unique_impl(value) else {
                panic!("key already present in map");
            };
            index
        };

        let index = match self.hash {
            EntryHash::Key1(_) => EntryIndex::Key1(index),
            EntryHash::Key2(_) => EntryIndex::Key2(index),
        };

        // SAFETY: map, as well as anything that was borrowed from it, is
        // dropped once the above block exits.
        unsafe { OccupiedEntry::new(self.map.0, index) }
    }
}

/// A view into an occupied entry in an [`IdHashMap`]. Part of the [`Entry`]
/// enum.
#[derive_where(Debug)]
pub struct OccupiedEntry<'a, T: BiHashItem> {
    map: DebugIgnore<DormantMutRef<'a, BiHashMap<T>>>,
    index: EntryIndex,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum EntryIndex {
    Key1(usize),
    Key2(usize),
}

impl EntryIndex {
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Key1(index) => *index,
            Self::Key2(index) => *index,
        }
    }
}

impl<'a, T: BiHashItem> OccupiedEntry<'a, T> {
    /// # Safety
    ///
    /// After self is created, the original reference created by
    /// `DormantMutRef::new` must not be used.
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T>>,
        index: EntryIndex,
    ) -> Self {
        OccupiedEntry { map: map.into(), index }
    }

    /// Gets a reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_ref`](Self::into_ref).
    pub fn get(&self) -> &T {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        unsafe { self.map.reborrow_shared() }
            .get_by_index(self.index.index())
            .expect("index is known to be valid")
    }

    /// Gets a mutable reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_mut`](Self::into_mut).
    pub fn get_mut(&mut self) -> RefMut<'_, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        unsafe { self.map.reborrow() }
            .get_by_index_mut(self.index.index())
            .expect("index is known to be valid")
    }

    /// Converts self into a reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get`](Self::get).
    pub fn into_ref(self) -> &'a T {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        unsafe { self.map.0.awaken() }
            .get_by_index(self.index.index())
            .expect("index is known to be valid")
    }

    /// Converts self into a mutable reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get_mut`](Self::get_mut).
    pub fn into_mut(self) -> RefMut<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        unsafe { self.map.0.awaken() }
            .get_by_index_mut(self.index.index())
            .expect("index is known to be valid")
    }

    /// Sets the entry to a new value, returning all values that conflict.
    ///
    /// # Panics
    ///
    /// Panics if the passed-in key is different from the key of the entry.
    pub fn insert(&mut self, value: T) -> Vec<T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        //
        // Note that `replace_at_index` panics if the keys don't match.
        unsafe { self.map.reborrow() }.replace_at_index(self.index, value)
    }

    /// Takes ownership of the value from the map.
    pub fn remove(mut self) -> T {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        unsafe { self.map.reborrow() }
            .remove_by_index(self.index.index())
            .expect("index is known to be valid")
    }
}
