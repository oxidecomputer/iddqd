use super::{BiHashItem, BiHashMap, RefMut, entry_indexes::EntryIndexes};
use crate::{
    DefaultHashBuilder,
    support::{
        alloc::{Allocator, Global},
        borrow::DormantMutRef,
        map_hash::MapHash,
    },
};
use alloc::vec::Vec;
use core::{fmt, hash::BuildHasher};

/// An implementation of the Entry API for [`BiHashMap`].
///
/// # Differences from single-key entries
///
/// The shape of this type differs from those provided for the other map types,
/// because it is possible for one of the two keys provided to correspond to an
/// existing entry, while the other does not.
///
/// [`VacantEntry`] corresponds to situations where neither key is present. To
/// insert an entry corresponding to the two keys, use [`VacantEntry::insert`].
///
/// [`OccupiedEntry`] represents situations where either the keys correspond to
/// different entries, or where only one of the keys is present. It provides the
/// following methods:
///
/// * [`OccupiedEntry::is_unique`] and [`OccupiedEntry::is_non_unique`] return
///   `true` if the keys correspond to a unique or duplicate entry in the map,
///   respectively.
/// * [`OccupiedEntry::get`] returns an [`OccupiedEntryRef`] enum that can be
///   matched on.
///   * [`OccupiedEntryRef::as_unique`] returns the unique entry, if one exists.
///   * [`OccupiedEntryRef::by_key1`] and [`OccupiedEntryRef::by_key2`] return the
///     entry corresponding to the given key, if one exists.
/// * Similarly, [`OccupiedEntry::get_mut`] returns an [`OccupiedEntryMut`] enum
///   that can be matched on.
///   * [`OccupiedEntryMut::as_unique`] returns a mutable reference to the unique
///     entry, if one exists.
///   * [`OccupiedEntryMut::by_key1`] and [`OccupiedEntryMut::by_key2`] return a
///     mutable reference to the entry corresponding to the given key, if one
///     exists.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{BiHashItem, BiHashMap, bi_hash_map, bi_upcast};
///
/// #[derive(Debug, PartialEq, Eq)]
/// struct Item {
///     id: u32,
///     name: String,
///     value: i32,
/// }
///
/// impl BiHashItem for Item {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///
///     fn key1(&self) -> Self::K1<'_> {
///         self.id
///     }
///     fn key2(&self) -> Self::K2<'_> {
///         &self.name
///     }
///     bi_upcast!();
/// }
///
/// let mut map = BiHashMap::new();
/// map.insert_unique(Item { id: 1, name: "foo".to_string(), value: 42 })
///     .unwrap();
///
/// // Get an existing entry. Both keys point to the same item, so the
/// // entry is unique.
/// match map.entry(1, "foo") {
///     bi_hash_map::Entry::Occupied(entry) => {
///         assert!(entry.is_unique());
///         assert_eq!(entry.get().as_unique().unwrap().value, 42);
///     }
///     bi_hash_map::Entry::Vacant(_) => panic!("Should be occupied"),
/// }
///
/// // Try to get a non-existing entry.
/// match map.entry(2, "bar") {
///     bi_hash_map::Entry::Occupied(_) => panic!("Should be vacant"),
///     bi_hash_map::Entry::Vacant(entry) => {
///         entry.insert(Item { id: 2, name: "bar".to_string(), value: 99 });
///     }
/// }
///
/// assert_eq!(map.len(), 2);
///
/// // An entry is non-unique when its two keys point to different items.
/// // Here, id 1 belongs to "foo" but name "bar" belongs to id 2.
/// match map.entry(1, "bar") {
///     bi_hash_map::Entry::Occupied(entry) => {
///         assert!(entry.is_non_unique());
///         let entry_ref = entry.get();
///         assert_eq!(entry_ref.by_key1().unwrap().name, "foo");
///         assert_eq!(entry_ref.by_key2().unwrap().id, 2);
///         assert_eq!(entry_ref.as_unique(), None);
///     }
///     bi_hash_map::Entry::Vacant(_) => panic!("Should be occupied"),
/// }
///
/// // An entry is also non-unique when only one of its keys is present.
/// match map.entry(1, "nonexistent") {
///     bi_hash_map::Entry::Occupied(mut entry) => {
///         assert!(entry.is_non_unique());
///         let entry_ref = entry.get();
///         assert_eq!(entry_ref.by_key1().unwrap().id, 1);
///         assert_eq!(entry_ref.by_key2(), None);
///
///         // Inserting overwrites whichever items the keys matched,
///         // returning them. Only id 1 ("foo") was present, so it alone
///         // is returned.
///         let replaced = entry.insert(Item {
///             id: 1,
///             name: "nonexistent".to_string(),
///             value: 7,
///         });
///         assert_eq!(replaced.len(), 1);
///         assert_eq!(replaced[0].name, "foo");
///
///         // The entry is now unique: both keys point to the new item.
///         assert!(entry.is_unique());
///         assert_eq!(entry.get().as_unique().unwrap().value, 7);
///     }
///     bi_hash_map::Entry::Vacant(_) => panic!("Should be occupied"),
/// }
///
/// // "foo" was overwritten in place, so the map still holds two items.
/// assert_eq!(map.get1(&1).unwrap().name, "nonexistent");
/// assert_eq!(map.get2(&"foo"), None);
/// assert_eq!(map.len(), 2);
/// # }
/// ```
pub enum Entry<'a, T: BiHashItem, S = DefaultHashBuilder, A: Allocator = Global>
{
    /// A vacant entry: none of the provided keys are present.
    Vacant(VacantEntry<'a, T, S, A>),
    /// An occupied entry where at least one of the keys is present in the map.
    Occupied(OccupiedEntry<'a, T, S, A>),
}

impl<'a, T: BiHashItem, S, A: Allocator> fmt::Debug for Entry<'a, T, S, A> {
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

impl<'a, T: BiHashItem, S: Clone + BuildHasher, A: Allocator>
    Entry<'a, T, S, A>
{
    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    ///
    /// # Panics
    ///
    /// Panics if the key hashes to a different value than the one passed
    /// into [`BiHashMap::entry`].
    #[inline]
    pub fn or_insert(self, default: T) -> OccupiedEntryMut<'a, T, S> {
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
    ) -> OccupiedEntryMut<'a, T, S> {
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
        F: FnMut(RefMut<'_, T, S>),
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
pub struct VacantEntry<
    'a,
    T: BiHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, BiHashMap<T, S, A>>,
    hashes: [MapHash; 2],
}

impl<'a, T: BiHashItem, S, A: Allocator> fmt::Debug
    for VacantEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry")
            .field("hashes", &self.hashes)
            .finish_non_exhaustive()
    }
}

impl<'a, T: BiHashItem, S: Clone + BuildHasher, A: Allocator>
    VacantEntry<'a, T, S, A>
{
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T, S, A>>,
        hashes: [MapHash; 2],
    ) -> Self {
        VacantEntry { map, hashes }
    }

    /// Sets the entry to a new value, returning a mutable reference to the
    /// value.
    pub fn insert(self, value: T) -> RefMut<'a, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.awaken() };
        let state = &map.tables.state;
        if !self.hashes[0].is_same_hash(state, value.key1()) {
            panic!("key1 hashes do not match");
        }
        if !self.hashes[1].is_same_hash(state, value.key2()) {
            panic!("key2 hashes do not match");
        }
        let Ok(index) = map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        map.get_by_index_mut(index).expect("index is known to be valid")
    }

    /// Sets the value of the entry, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(mut self, value: T) -> OccupiedEntry<'a, T, S, A> {
        let index = {
            // SAFETY: The safety assumption behind `Self::new` guarantees that the
            // original reference to the map is not used at this point.
            let map = unsafe { self.map.reborrow() };
            let state = &map.tables.state;
            if !self.hashes[0].is_same_hash(state, value.key1()) {
                panic!("key1 hashes do not match");
            }
            if !self.hashes[1].is_same_hash(state, value.key2()) {
                panic!("key2 hashes do not match");
            }
            let Ok(index) = map.insert_unique_impl(value) else {
                panic!("key already present in map");
            };
            index
        };

        // SAFETY: map, as well as anything that was borrowed from it, is
        // dropped once the above block exits.
        unsafe { OccupiedEntry::new(self.map, EntryIndexes::Unique(index)) }
    }
}

/// A view into an occupied entry in a [`BiHashMap`]. Part of the [`Entry`]
/// enum.
pub struct OccupiedEntry<
    'a,
    T: BiHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, BiHashMap<T, S, A>>,
    indexes: EntryIndexes,
}

impl<'a, T: BiHashItem, S, A: Allocator> fmt::Debug
    for OccupiedEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("indexes", &self.indexes)
            .finish_non_exhaustive()
    }
}

impl<'a, T: BiHashItem, S: Clone + BuildHasher, A: Allocator>
    OccupiedEntry<'a, T, S, A>
{
    /// # Safety
    ///
    /// After self is created, the original reference created by
    /// `DormantMutRef::new` must not be used.
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, BiHashMap<T, S, A>>,
        indexes: EntryIndexes,
    ) -> Self {
        OccupiedEntry { map, indexes }
    }

    /// Returns true if the entry is unique.
    ///
    /// Since [`BiHashMap`] is keyed by two keys, it's possible for
    /// `OccupiedEntry` to match up to two separate items. This function returns
    /// true if the entry is unique, meaning all keys point to exactly one item.
    pub fn is_unique(&self) -> bool {
        self.indexes.is_unique()
    }

    /// Returns true if the `OccupiedEntry` represents more than one item, or if
    /// some keys are not present.
    #[inline]
    pub fn is_non_unique(&self) -> bool {
        !self.is_unique()
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
    pub fn get_mut(&mut self) -> OccupiedEntryMut<'_, T, S> {
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
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index(self.indexes)
    }

    /// Converts self into mutable references to items that match the provided
    /// keys.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see
    /// [`get_mut`](Self::get_mut).
    pub fn into_mut(self) -> OccupiedEntryMut<'a, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is not used at this point.
        let map = unsafe { self.map.awaken() };
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
    NonUnique {
        /// The value fetched by the first key.
        by_key1: Option<&'a T>,

        /// The value fetched by the second key.
        by_key2: Option<&'a T>,
    },
}

impl<'a, T: BiHashItem> OccupiedEntryRef<'a, T> {
    /// Returns true if the entry is unique.
    ///
    /// Since [`BiHashMap`] is keyed by two keys, it's possible for
    /// `OccupiedEntry` to match up to two separate items. This function returns
    /// true if the entry is unique, meaning all keys point to exactly one item.
    #[inline]
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::Unique(_))
    }

    /// Returns true if the `OccupiedEntryRef` represents more than one item, or
    /// if some keys are not present.
    #[inline]
    pub fn is_non_unique(&self) -> bool {
        matches!(self, Self::NonUnique { .. })
    }

    /// Returns a reference to the value if it is unique.
    #[inline]
    pub fn as_unique(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::NonUnique { .. } => None,
        }
    }

    /// Returns a reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::NonUnique { by_key1, .. } => *by_key1,
        }
    }

    /// Returns a reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::NonUnique { by_key2, .. } => *by_key2,
        }
    }
}

/// A mutable view into an occupied entry in a [`BiHashMap`].
///
/// Returned by [`OccupiedEntry::get_mut`].
pub enum OccupiedEntryMut<
    'a,
    T: BiHashItem,
    S: Clone + BuildHasher = DefaultHashBuilder,
> {
    /// All keys point to the same entry.
    Unique(RefMut<'a, T, S>),

    /// The keys point to different entries, or some keys are not present.
    NonUnique {
        /// The value fetched by the first key.
        by_key1: Option<RefMut<'a, T, S>>,

        /// The value fetched by the second key.
        by_key2: Option<RefMut<'a, T, S>>,
    },
}

impl<'a, T: BiHashItem + fmt::Debug, S: Clone + BuildHasher> fmt::Debug
    for OccupiedEntryMut<'a, T, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OccupiedEntryMut::Unique(ref_mut) => {
                f.debug_tuple("Unique").field(ref_mut).finish()
            }
            OccupiedEntryMut::NonUnique { by_key1, by_key2 } => f
                .debug_struct("NonUnique")
                .field("by_key1", by_key1)
                .field("by_key2", by_key2)
                .finish(),
        }
    }
}

impl<'a, T: BiHashItem, S: Clone + BuildHasher> OccupiedEntryMut<'a, T, S> {
    /// Returns true if the entry is unique.
    #[inline]
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::Unique(_))
    }

    /// Returns true if the `OccupiedEntryMut` represents more than one item, or
    /// if some keys are not present.
    #[inline]
    pub fn is_non_unique(&self) -> bool {
        matches!(self, Self::NonUnique { .. })
    }

    /// Returns a reference to the value if it is unique.
    #[inline]
    pub fn as_unique(&mut self) -> Option<RefMut<'_, T, S>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::NonUnique { .. } => None,
        }
    }

    /// Returns a mutable reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&mut self) -> Option<RefMut<'_, T, S>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::NonUnique { by_key1, .. } => {
                by_key1.as_mut().map(|v| v.reborrow())
            }
        }
    }

    /// Returns a mutable reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&mut self) -> Option<RefMut<'_, T, S>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::NonUnique { by_key2, .. } => {
                by_key2.as_mut().map(|v| v.reborrow())
            }
        }
    }

    /// Calls a callback for each value.
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(RefMut<'_, T, S>),
    {
        match self {
            Self::Unique(v) => f(v.reborrow()),
            Self::NonUnique { by_key1, by_key2 } => {
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
