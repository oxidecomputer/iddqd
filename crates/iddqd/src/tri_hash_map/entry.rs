use super::{
    TriHashItem, TriHashMap, entry_indexes::EntryIndexes, ref_mut::RefMut,
};
use crate::{
    DefaultHashBuilder,
    support::{
        alloc::{Allocator, Global},
        borrow::DormantMutRef,
        map_hash::MapHash,
    },
};
use core::{fmt, hash::BuildHasher};

/// An implementation of the Entry API for [`TriHashMap`].
///
/// A vacant entry means none of the three provided keys are present. An
/// occupied entry is unique only when all three keys point to the same item;
/// partial matches and mixed matches are occupied non-unique entries.
pub enum Entry<
    'a,
    T: TriHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    /// A vacant entry: none of the provided keys are present.
    Vacant(VacantEntry<'a, T, S, A>),
    /// An occupied entry where at least one of the keys is present in the map.
    Occupied(OccupiedEntry<'a, T, S, A>),
}

impl<'a, T: TriHashItem, S, A: Allocator> fmt::Debug for Entry<'a, T, S, A> {
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

/// A vacant entry.
pub struct VacantEntry<
    'a,
    T: TriHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, TriHashMap<T, S, A>>,
    hashes: [MapHash; 3],
}

impl<'a, T: TriHashItem, S, A: Allocator> fmt::Debug
    for VacantEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry")
            .field("hashes", &self.hashes)
            .finish_non_exhaustive()
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher, A: Allocator>
    VacantEntry<'a, T, S, A>
{
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, TriHashMap<T, S, A>>,
        hashes: [MapHash; 3],
    ) -> Self {
        VacantEntry { map, hashes }
    }

    /// Sets the entry to a new value, returning a mutable reference to it.
    ///
    /// # Panics
    ///
    /// Panics before mutation if any value key hashes differently from the
    /// corresponding key passed to [`TriHashMap::entry`].
    pub fn insert(self, value: T) -> RefMut<'a, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point.
        let map = unsafe { self.map.awaken() };
        validate_hashes(map, self.hashes, &value);
        let Ok(index) = map.insert_unique_impl(value) else {
            panic!("key already present in map");
        };
        map.get_by_index_mut(index).expect("index is known to be valid")
    }

    /// Sets the entry to a new value, and returns an `OccupiedEntry`.
    #[inline]
    pub fn insert_entry(mut self, value: T) -> OccupiedEntry<'a, T, S, A> {
        let index = {
            // SAFETY: The safety assumption behind `Self::new` guarantees that
            // the original reference to the map is no longer used at this
            // point.
            let map = unsafe { self.map.reborrow() };
            validate_hashes(map, self.hashes, &value);
            let Ok(index) = map.insert_unique_impl(value) else {
                panic!("key already present in map");
            };
            index
        };

        // SAFETY: `map`, as well as anything borrowed from it, is dropped
        // above, so the temporary reborrow has ended before awakening again.
        unsafe { OccupiedEntry::new(self.map, EntryIndexes::Unique(index)) }
    }
}

fn validate_hashes<T: TriHashItem, S: Clone + BuildHasher, A: Allocator>(
    map: &TriHashMap<T, S, A>,
    hashes: [MapHash; 3],
    value: &T,
) {
    let state = &map.tables.state;
    if !hashes[0].is_same_hash(state, value.key1()) {
        panic!("key1 hashes do not match");
    }
    if !hashes[1].is_same_hash(state, value.key2()) {
        panic!("key2 hashes do not match");
    }
    if !hashes[2].is_same_hash(state, value.key3()) {
        panic!("key3 hashes do not match");
    }
}

/// A view into an occupied entry in a [`TriHashMap`].
pub struct OccupiedEntry<
    'a,
    T: TriHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    map: DormantMutRef<'a, TriHashMap<T, S, A>>,
    indexes: EntryIndexes,
}

impl<'a, T: TriHashItem, S, A: Allocator> fmt::Debug
    for OccupiedEntry<'a, T, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("indexes", &self.indexes)
            .finish_non_exhaustive()
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher, A: Allocator>
    OccupiedEntry<'a, T, S, A>
{
    /// # Safety
    ///
    /// After self is created, the original reference created by
    /// `DormantMutRef::new` must not be used.
    pub(super) unsafe fn new(
        map: DormantMutRef<'a, TriHashMap<T, S, A>>,
        indexes: EntryIndexes,
    ) -> Self {
        OccupiedEntry { map, indexes }
    }

    /// Returns true if all three keys point to exactly one item.
    #[inline]
    pub fn is_unique(&self) -> bool {
        self.indexes.is_unique()
    }

    /// Returns true if more than one item matched, or if some keys are absent.
    #[inline]
    pub fn is_non_unique(&self) -> bool {
        !self.is_unique()
    }

    /// Returns shared references to values that match the provided keys.
    pub fn get(&self) -> OccupiedEntryRef<'_, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.reborrow_shared() };
        map.get_by_entry_index(self.indexes)
    }

    /// Converts self into shared references to items that match the provided keys.
    pub fn into_ref(self) -> OccupiedEntryRef<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index(self.indexes)
    }
}

/// Shared references to values matched by a [`TriHashMap`] occupied entry.
#[derive(Debug)]
pub enum OccupiedEntryRef<'a, T: TriHashItem> {
    /// All keys point to the same entry.
    Unique(&'a T),
    /// The keys point to different entries, or some keys are not present.
    NonUnique(NonUniqueEntryRef<'a, T>),
}

/// Accessor-backed shared non-unique entry references.
#[derive(Debug)]
pub struct NonUniqueEntryRef<'a, T: TriHashItem> {
    values: [Option<&'a T>; 3],
    len: usize,
    key_to_slot: [Option<usize>; 3],
}

impl<'a, T: TriHashItem> OccupiedEntryRef<'a, T> {
    /// Returns true if all three keys point to exactly one item.
    #[inline]
    pub fn is_unique(&self) -> bool {
        matches!(self, Self::Unique(_))
    }
    /// Returns true if more than one item matched, or if some keys are absent.
    #[inline]
    pub fn is_non_unique(&self) -> bool {
        matches!(self, Self::NonUnique(_))
    }
    /// Returns a reference to the value if the entry is unique.
    #[inline]
    pub fn as_unique(&self) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::NonUnique(_) => None,
        }
    }
    /// Returns a reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&self) -> Option<&'a T> {
        self.by_key(0)
    }
    /// Returns a reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&self) -> Option<&'a T> {
        self.by_key(1)
    }
    /// Returns a reference to the value fetched by the third key.
    #[inline]
    pub fn by_key3(&self) -> Option<&'a T> {
        self.by_key(2)
    }
    fn by_key(&self, key: usize) -> Option<&'a T> {
        match self {
            Self::Unique(v) => Some(v),
            Self::NonUnique(non_unique) => non_unique.by_key(key),
        }
    }
    /// Calls `f` once for each distinct matched value in first-key-hit order.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&'a T),
    {
        match self {
            Self::Unique(v) => f(v),
            Self::NonUnique(non_unique) => non_unique.for_each(f),
        }
    }
}

impl<'a, T: TriHashItem> NonUniqueEntryRef<'a, T> {
    pub(super) fn new(
        values: [Option<&'a T>; 3],
        len: usize,
        key_to_slot: [Option<usize>; 3],
    ) -> Self {
        Self { values, len, key_to_slot }
    }

    /// Returns a reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&self) -> Option<&'a T> {
        self.by_key(0)
    }

    /// Returns a reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&self) -> Option<&'a T> {
        self.by_key(1)
    }

    /// Returns a reference to the value fetched by the third key.
    #[inline]
    pub fn by_key3(&self) -> Option<&'a T> {
        self.by_key(2)
    }

    #[inline]
    fn by_key(&self, key: usize) -> Option<&'a T> {
        self.key_to_slot[key].and_then(|slot| self.values[slot])
    }

    /// Calls `f` once for each distinct matched value in first-key-hit order.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&'a T),
    {
        for value in self.values[..self.len].iter().flatten() {
            f(value);
        }
    }
}
