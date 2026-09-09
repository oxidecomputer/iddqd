use super::{IdOrdItem, IdOrdMap, RefMut};
use crate::support::ItemIndex;
use core::{fmt, hash::Hash};

/// An implementation of the Entry API for [`IdOrdMap`].
pub enum Entry<'a, T: IdOrdItem> {
    /// A vacant entry.
    Vacant(VacantEntry<'a, T>),
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, T>),
}

impl<'a, T: IdOrdItem> fmt::Debug for Entry<'a, T> {
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

impl<'a, T: IdOrdItem> Entry<'a, T> {
    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a shared reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key is already present in the map. (The intention is that
    /// the key should be what was passed into [`IdOrdMap::entry`], but that
    /// isn't checked in this API due to borrow checker limitations.)
    #[inline]
    pub fn or_insert_ref(self, default: T) -> &'a T {
        match self {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => entry.insert_ref(default),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key is already present in the map. (The intention is that
    /// the key should be what was passed into [`IdOrdMap::entry`], but that
    /// isn't checked in this API due to borrow checker limitations.)
    #[inline]
    pub fn or_insert(self, default: T) -> RefMut<'a, T>
    where
        T::Key<'a>: Hash,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a shared reference to the value in the
    /// entry.
    ///
    /// # Panics
    ///
    /// Panics if the key is already present in the map. (The intention is that
    /// the key should be what was passed into [`IdOrdMap::entry`], but that
    /// isn't checked in this API due to borrow checker limitations.)
    #[inline]
    pub fn or_insert_with_ref<F: FnOnce() -> T>(self, default: F) -> &'a T {
        match self {
            Entry::Occupied(entry) => entry.into_ref(),
            Entry::Vacant(entry) => entry.insert_ref(default()),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    ///
    /// # Panics
    ///
    /// Panics if the key is already present in the map. (The intention is that
    /// the key should be what was passed into [`IdOrdMap::entry`], but that
    /// isn't checked in this API due to borrow checker limitations.)
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> T>(self, default: F) -> RefMut<'a, T>
    where
        T::Key<'a>: Hash,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    ///
    /// # Notes
    ///
    /// Due to limitations in current versions of Rust, this method can only be
    /// called if `T: 'static`. See the ["Key lifetimes"](RefMut#key-lifetimes)
    /// section in [`RefMut`] for more details.
    ///
    /// For maps with borrowed keys, a suggested alternative is to match on the
    /// entry's variants, calling `get_mut` on an `OccupiedEntry`.
    ///
    /// # Panics
    ///
    /// Panics if `f` changes the item's key, as detected by the `RefMut`.
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(RefMut<'_, T>),
        for<'k> T::Key<'k>: Hash,
    {
        match self {
            Entry::Occupied(entry) => {
                {
                    let map = &mut *entry.map;
                    let state = map.tables.state().clone();
                    let item =
                        map.items.get_mut(entry.index).expect("index is valid");
                    let hash = map.tables.make_hash(&*item);
                    f(RefMut::new(state, hash, item));
                }
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

/// A vacant entry.
pub struct VacantEntry<'a, T: IdOrdItem> {
    map: &'a mut IdOrdMap<T>,
}

impl<'a, T: IdOrdItem> fmt::Debug for VacantEntry<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry").finish_non_exhaustive()
    }
}

impl<'a, T: IdOrdItem> VacantEntry<'a, T> {
    pub(super) fn new(map: &'a mut IdOrdMap<T>) -> Self {
        VacantEntry { map }
    }

    /// Sets the entry to a new value, returning a shared reference to the
    /// value.
    ///
    /// # Panics
    ///
    /// Panics if the key is already present in the map. (The intention is that
    /// the key should be what was passed into [`IdOrdMap::entry`], but that
    /// isn't checked in this API due to borrow checker limitations.)
    pub fn insert_ref(self, value: T) -> &'a T {
        let map = self.map;
        let Ok(index) = map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        map.get_by_index(index).expect("index is known to be valid")
    }

    /// Sets the entry to a new value, returning a mutable reference to the
    /// value.
    pub fn insert(self, value: T) -> RefMut<'a, T>
    where
        T::Key<'a>: Hash,
    {
        let map = self.map;
        let Ok(index) = map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        map.get_by_index_mut(index).expect("index is known to be valid")
    }

    /// Sets the value of the entry, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(self, value: T) -> OccupiedEntry<'a, T> {
        let Ok(index) = self.map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        OccupiedEntry::new(self.map, index)
    }
}

/// A view into an occupied entry in an [`IdOrdMap`]. Part of the [`Entry`]
/// enum.
pub struct OccupiedEntry<'a, T: IdOrdItem> {
    map: &'a mut IdOrdMap<T>,
    // index is a valid index into the map's internal hash table.
    index: ItemIndex,
}

impl<'a, T: IdOrdItem> fmt::Debug for OccupiedEntry<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<'a, T: IdOrdItem> OccupiedEntry<'a, T> {
    pub(super) fn new(map: &'a mut IdOrdMap<T>, index: ItemIndex) -> Self {
        OccupiedEntry { map, index }
    }

    /// Gets a reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_ref`](Self::into_ref).
    pub fn get(&self) -> &T {
        self.map.get_by_index(self.index).expect("index is known to be valid")
    }

    /// Gets a mutable reference to the value.
    ///
    /// If you need a reference to `T` that may outlive the destruction of the
    /// `Entry` value, see [`into_mut`](Self::into_mut).
    pub fn get_mut<'b>(&'b mut self) -> RefMut<'b, T>
    where
        T::Key<'b>: Hash,
    {
        self.map
            .get_by_index_mut(self.index)
            .expect("index is known to be valid")
    }

    /// Converts self into a reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get`](Self::get).
    pub fn into_ref(self) -> &'a T {
        self.map.get_by_index(self.index).expect("index is known to be valid")
    }

    /// Converts self into a mutable reference to the value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get_mut`](Self::get_mut).
    pub fn into_mut(self) -> RefMut<'a, T>
    where
        T::Key<'a>: Hash,
    {
        self.map
            .get_by_index_mut(self.index)
            .expect("index is known to be valid")
    }

    /// Sets the entry to a new value, returning the old value.
    ///
    /// # Panics
    ///
    /// Panics if `value.key()` is different from the key of the entry.
    pub fn insert(&mut self, value: T) -> T {
        // Note that `replace_at_index` panics if the keys don't match.
        self.map.replace_at_index(self.index, value)
    }

    /// Takes ownership of the value from the map.
    pub fn remove(self) -> T {
        self.map
            .remove_by_index(self.index)
            .expect("index is known to be valid")
    }
}
