// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::{entry_indexes::EntryIndexes, BiHashItem, BiHashMap, RefMut};
use crate::support::{borrow::DormantMutRef, map_hash::MapHash};
use debug_ignore::DebugIgnore;
use derive_where::derive_where;

/// An implementation of the Entry API for [`BiHashMap`].
#[derive_where(Debug)]
pub enum Entry<'a, T: BiHashItem> {
    /// A vacant entry: none of the provided keys are present.
    Vacant(VacantEntry<'a, T>),
    /// An occupied entry where at least one of the keys is present in the map.
    Occupied(OccupiedEntry<'a, T>),
}

impl<'a, T: BiHashItem> Entry<'a, T> {
    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`BiHashMap::entry`].
    #[inline]
    pub fn or_insert(self, default: T) -> OccupiedEntryMut<'a, T> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                OccupiedEntryMut::Unique(entry.insert(default))
            }
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`BiHashMap::entry`].
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> T>(
        self,
        default: F,
    ) -> OccupiedEntryMut<'a, T> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                OccupiedEntryMut::Unique(entry.insert(default()))
            }
        }
    }

    /// Provides in-place mutable access to occupied entries before any
    /// potential inserts into the map.
    ///
    /// `F` is called for each entry that matches the provided keys.
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnMut(RefMut<'_, T>),
    {
        match self {
            Entry::Occupied(mut entry) => {
                entry.get_mut().for_each(f);
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

/// A vacant entry.
#[derive_where(Debug)]
pub struct VacantEntry<'a, T: BiHashItem> {
    map: DebugIgnore<DormantMutRef<'a, BiHashMap<T>>>,
    hashes: [MapHash; 2],
}

impl<'a, T: BiHashItem> VacantEntry<'a, T> {
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T>>,
        hashes: [MapHash; 2],
    ) -> Self {
        VacantEntry { map: map.into(), hashes }
    }

    /// Sets the entry to a new value, returning a mutable reference to the
    /// value.
    pub fn insert(self, value: T) -> RefMut<'a, T> {
        if !self.hashes[0].is_same_hash(value.key1()) {
            panic!("key1 hashes do not match");
        }
        if !self.hashes[1].is_same_hash(value.key2()) {
            panic!("key2 hashes do not match");
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
        if !self.hashes[0].is_same_hash(value.key1()) {
            panic!("key1 hashes do not match");
        }
        if !self.hashes[1].is_same_hash(value.key2()) {
            panic!("key2 hashes do not match");
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

        // SAFETY: map, as well as anything that was borrowed from it, is
        // dropped once the above block exits.
        unsafe { OccupiedEntry::new(self.map.0, EntryIndexes::Unique(index)) }
    }
}

/// A view into an occupied entry in a [`BiHashMap`]. Part of the [`Entry`]
/// enum.
#[derive_where(Debug)]
pub struct OccupiedEntry<'a, T: BiHashItem> {
    map: DebugIgnore<DormantMutRef<'a, BiHashMap<T>>>,
    indexes: EntryIndexes,
}

impl<'a, T: BiHashItem> OccupiedEntry<'a, T> {
    /// # Safety
    ///
    /// After self is created, the original reference created by
    /// `DormantMutRef::new` must not be used.
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T>>,
        indexes: EntryIndexes,
    ) -> Self {
        OccupiedEntry { map: map.into(), indexes }
    }

    /// Returns true if this is a unique entry.
    ///
    /// Since [`BiHashMap`] is keyed by two keys, it's possible for
    /// `OccupiedEntry` to match up to two separate entries. This function
    /// returns true if the entry is unique, meaning it only matches one entry.
    pub fn is_unique(&self) -> bool {
        self.indexes.is_unique()
    }

    /// Returns references to values that match the provided keys.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_ref`](Self::into_ref).
    pub fn get(&self) -> OccupiedEntryRef<'_, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.reborrow_shared() };
        map.get_by_entry_index(self.indexes)
    }

    /// Returns mutable references to values that match the provided keys.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_mut`](Self::into_mut).
    pub fn get_mut(&mut self) -> OccupiedEntryMut<'_, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.reborrow() };
        map.get_by_entry_index_mut(self.indexes)
    }

    /// Converts self into shared references to items that match the provided
    /// keys.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get`](Self::get).
    pub fn into_ref(self) -> OccupiedEntryRef<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.0.awaken() };
        map.get_by_entry_index(self.indexes)
    }

    /// Converts self into mutable references to items that match the provided
    /// keys.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get_mut`](Self::get_mut).
    pub fn into_mut(self) -> OccupiedEntryMut<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.0.awaken() };
        map.get_by_entry_index_mut(self.indexes)
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
        // Note that `replace_at_indexes` panics if the keys don't match.
        let map = unsafe { self.map.reborrow() };
        let (index, old_items) = map.replace_at_indexes(self.indexes, value);
        self.indexes = EntryIndexes::Unique(index);
        old_items
    }

    /// Takes ownership of the values from the map.
    pub fn remove(mut self) -> Vec<T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.reborrow() };
        map.remove_by_entry_index(self.indexes)
    }
}

/// A view into an occupied entry in a [`BiHashMap`].
///
/// Returned by [`OccupiedEntry::get`].
#[derive(Debug)]
pub enum OccupiedEntryRef<'a, T: BiHashItem> {
    /// All keys point to the same entry.
    Unique(&'a T),

    /// The keys point to different entries, or some keys are not present.
    ///
    /// At least one of `by_key1` and `by_key2` is `Some`.
    Multiple {
        /// The value fetched by the first key.
        by_key1: Option<&'a T>,

        /// The value fetched by the second key.
        by_key2: Option<&'a T>,
    },
}

impl<'a, T: BiHashItem> OccupiedEntryRef<'a, T> {
    /// Returns true if the entry is unique.
    #[inline]
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::Unique(_))
    }

    /// Returns true if the `OccupiedEntryRef` represents more than one item, or
    /// if some keys are not present.
    #[inline]
    pub fn is_multiple(&self) -> bool {
        matches!(self, Self::Multiple { .. })
    }

    /// Returns a reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::Multiple { by_key1, .. } => *by_key1,
        }
    }

    /// Returns a reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::Multiple { by_key2, .. } => *by_key2,
        }
    }
}

/// A mutable view into an occupied entry in a [`BiHashMap`].
///
/// Returned by [`OccupiedEntry::get_mut`].
#[derive(Debug)]
pub enum OccupiedEntryMut<'a, T: BiHashItem> {
    /// All keys point to the same entry.
    Unique(RefMut<'a, T>),

    /// The keys point to different entries, or some keys are not present.
    Multiple {
        /// The value fetched by the first key.
        by_key1: Option<RefMut<'a, T>>,

        /// The value fetched by the second key.
        by_key2: Option<RefMut<'a, T>>,
    },
}

impl<'a, T: BiHashItem> OccupiedEntryMut<'a, T> {
    /// Returns true if the entry is unique.
    #[inline]
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::Unique(_))
    }

    /// Returns true if the `OccupiedEntryMut` represents more than one item, or
    /// if some keys are not present.
    #[inline]
    pub fn is_multiple(&self) -> bool {
        matches!(self, Self::Multiple { .. })
    }

    /// Returns a mutable reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&mut self) -> Option<RefMut<'_, T>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::Multiple { by_key1, .. } => {
                by_key1.as_mut().map(|v| v.reborrow())
            }
        }
    }

    /// Returns a mutable reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&mut self) -> Option<RefMut<'_, T>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::Multiple { by_key2, .. } => {
                by_key2.as_mut().map(|v| v.reborrow())
            }
        }
    }

    /// Calls a callback for each value.
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(RefMut<'_, T>),
    {
        match self {
            Self::Unique(v) => f(v.reborrow()),
            Self::Multiple { by_key1, by_key2 } => {
                if let Some(v) = by_key1 {
                    f(v.reborrow());
                }
                if let Some(v) = by_key2 {
                    f(v.reborrow());
                }
            }
        }
    }
}

// pub struct OccupiedEntryIter<'a, T: BiHashItem> {
//     map: &'a BiHashMap<T>,
//     indexes: btree_set::Iter<'a, usize>,
// }

// impl<'a, T: BiHashItem> Iterator for OccupiedEntryIter<'a, T> {
//     type Item = &'a T;

//     fn next(&mut self) -> Option<Self::Item> {
//         let index = self.indexes.next()?;
//         self.map.get_by_index(*index)
//     }
// }

// impl<'a, T: BiHashItem> ExactSizeIterator for OccupiedEntryIter<'a, T> {
//     fn len(&self) -> usize {
//         self.indexes.len()
//     }
// }

// // btree_set::Iter is fused, so this is as well.
// impl<'a, T: BiHashItem> FusedIterator for OccupiedEntryIter<'a, T> {}

// pub struct OccupiedEntryIterMut<'a, T: BiHashItem> {
//     map: &'a mut BiHashMap<T>,
//     indexes: btree_set::Iter<'a, usize>,
// }

// impl<'a, T: BiHashItem> Iterator for OccupiedEntryIterMut<'a, T> {
//     type Item = RefMut<'a, T>;

//     fn next(&mut self) -> Option<Self::Item> {
//         let index = self.indexes.next()?;

//         let item = self
//             .map
//             .get_by_index_mut(*index)
//             .expect("index is known to be valid");

//         // SAFETY: This lifetime extension from self to 'a is safe based on two
//         // things:
//         //
//         // 1. We never repeat indexes, i.e. for an index i, once we've handed
//         //    out an item at i, creating `&mut T`, we'll never get the index i
//         //    again. (This is guaranteed from the set-based nature of the
//         //    iterator.) This means that we don't ever create a mutable alias to
//         //    the same memory.
//         //
//         // 2. All mutable references to data within self.map are derived from
//         //    self.map. So, the rule described at [1] is upheld:
//         //
//         //    > When creating a mutable reference, then while this reference
//         //    > exists, the memory it points to must not get accessed (read or
//         //    > written) through any other pointer or reference not derived from
//         //    > this reference.
//         //
//         // [1]:
//         //     https://doc.rust-lang.org/std/ptr/index.html#pointer-to-reference-conversion
//         let item = unsafe {
//             std::mem::transmute::<RefMut<'_, T>, RefMut<'a, T>>(item)
//         };
//         Some(item)
//     }
// }

// impl<'a, T: BiHashItem> ExactSizeIterator for OccupiedEntryIterMut<'a, T> {
//     fn len(&self) -> usize {
//         self.indexes.len()
//     }
// }

// // btree_set::Iter is fused, so this is as well.
// impl<'a, T: BiHashItem> FusedIterator for OccupiedEntryIterMut<'a, T> {}

// pub struct OccupiedEntryIntoIter<'a, T: BiHashItem> {
//     map: &'a mut BiHashMap<T>,
//     indexes: btree_set::IntoIter<usize>,
// }

// impl<'a, T: BiHashItem> Iterator for OccupiedEntryIntoIter<'a, T> {
//     type Item = RefMut<'a, T>;

//     fn next(&mut self) -> Option<Self::Item> {
//         let index = self.indexes.next()?;
//         let item = self
//             .map
//             .get_by_index_mut(index)
//             .expect("index is known to be valid");

//         // SAFETY: This lifetime extension from self to 'a is safe based on two
//         // things:
//         //
//         // 1. We never repeat indexes, i.e. for an index i, once we've handed
//         //    out an item at i, creating `&mut T`, we'll never get the index i
//         //    again. (This is guaranteed from the set-based nature of the
//         //    iterator.) This means that we don't ever create a mutable alias to
//         //    the same memory.
//         //
//         // 2. All mutable references to data within self.map are derived from
//         //    self.map. So, the rule described at [1] is upheld:
//         //
//         //    > When creating a mutable reference, then while this reference
//         //    > exists, the memory it points to must not get accessed (read or
//         //    > written) through any other pointer or reference not derived from
//         //    > this reference.
//         //
//         // [1]:
//         //     https://doc.rust-lang.org/std/ptr/index.html#pointer-to-reference-conversion
//         let item = unsafe {
//             std::mem::transmute::<RefMut<'_, T>, RefMut<'a, T>>(item)
//         };
//         Some(item)
//     }
// }

// impl<'a, T: BiHashItem> ExactSizeIterator for OccupiedEntryIntoIter<'a, T> {
//     fn len(&self) -> usize {
//         self.indexes.len()
//     }
// }

// // btree_set::IntoIter is fused, so this is as well.
// impl<'a, T: BiHashItem> FusedIterator for OccupiedEntryIntoIter<'a, T> {}
