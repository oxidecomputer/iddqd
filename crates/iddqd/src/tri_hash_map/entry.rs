use super::{
    TriHashItem, TriHashMap, entry_indexes::EntryIndexes, ref_mut::RefMut,
};
use crate::{
    DefaultHashBuilder,
    support::{
        ItemIndex,
        alloc::{Allocator, Global},
        borrow::DormantMutRef,
        map_hash::MapHash,
    },
};
use alloc::vec::Vec;
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

impl<'a, T: TriHashItem, S: Clone + BuildHasher, A: Allocator>
    Entry<'a, T, S, A>
{
    /// Provides in-place mutable access to occupied entries before any
    /// potential inserts into the map.
    ///
    /// `F` is called once for each distinct entry that matches the provided
    /// keys, in first-key-hit order. Vacant entries are left unchanged.
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

    /// Ensures a value is in the entry by inserting `value` if vacant, and
    /// returns mutable occupied access to the entry.
    #[inline]
    pub fn or_insert(self, value: T) -> OccupiedEntryMut<'a, T, S> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                OccupiedEntryMut::Unique(entry.insert(value))
            }
        }
    }

    /// Ensures a value is in the entry by inserting the result of `default` if
    /// vacant, and returns mutable occupied access to the entry.
    #[inline]
    pub fn or_insert_with<F>(self, default: F) -> OccupiedEntryMut<'a, T, S>
    where
        F: FnOnce() -> T,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                OccupiedEntryMut::Unique(entry.insert(default()))
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
        validate_hashes(map, self.hashes.clone(), &value);
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
            validate_hashes(map, self.hashes.clone(), &value);
            let Ok(index) = map.insert_unique_impl(value) else {
                panic!("key already present in map");
            };
            index
        };

        // SAFETY: `map`, as well as anything borrowed from it, is dropped
        // above, so the temporary reborrow has ended before awakening again.
        unsafe {
            OccupiedEntry::new(
                self.map,
                EntryIndexes::Unique(index),
                self.hashes,
            )
        }
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
    hashes: [MapHash; 3],
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
        hashes: [MapHash; 3],
    ) -> Self {
        OccupiedEntry { map, indexes, hashes }
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

    /// Returns mutable references to values that match the provided keys.
    pub fn get_mut(&mut self) -> OccupiedEntryMut<'_, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.reborrow() };
        map.get_by_entry_index_mut(self.indexes)
    }

    /// Converts self into shared references to items that match the provided keys.
    pub fn into_ref(self) -> OccupiedEntryRef<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index(self.indexes)
    }

    /// Converts self into mutable references to items that match the provided keys.
    pub fn into_mut(self) -> OccupiedEntryMut<'a, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index_mut(self.indexes)
    }

    /// Removes all distinct values matched by this entry.
    pub fn remove(self) -> Vec<T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point.
        let map = unsafe { self.map.awaken() };
        let duplicates = prepare_entry_removal(map, self.indexes);
        let mut removed = Vec::with_capacity(duplicates.len());
        map.remove_prepared_duplicates(duplicates, &mut removed);
        removed
    }

    /// Replaces all distinct values matched by this entry with `value`.
    ///
    /// # Panics
    ///
    /// Panics before mutation if `value` does not match the entry key hashes
    /// or if its duplicate state is incompatible with this entry.
    pub fn insert(&mut self, value: T) -> Vec<T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.reborrow() };
        validate_hashes(map, self.hashes.clone(), &value);
        let prepared = map.prepare_insert_overwrite(&value);
        validate_prepared_indexes(self.indexes, prepared.indexes);

        let mut removed = Vec::with_capacity(prepared.duplicate_count());
        map.try_reserve_insert_overwrite_commit(prepared.needs_new_item_slot())
            .expect("reserved capacity for entry replacement commit");
        let next_index =
            map.commit_insert_overwrite(value, prepared, &mut removed);
        self.indexes = EntryIndexes::Unique(next_index);
        removed
    }
}

fn validate_prepared_indexes(
    indexes: EntryIndexes,
    prepared: [Option<ItemIndex>; 3],
) {
    let expected = match indexes {
        EntryIndexes::Unique(index) => [Some(index), Some(index), Some(index)],
        EntryIndexes::NonUnique(indexes) => *indexes.indexes(),
    };
    if prepared != expected {
        panic!("replacement duplicate state does not match entry lookup");
    }
}

fn prepare_entry_removal<
    T: TriHashItem,
    S: Clone + BuildHasher,
    A: Allocator,
>(
    map: &TriHashMap<T, S, A>,
    indexes: EntryIndexes,
) -> Vec<super::imp::PreparedDuplicate> {
    let distinct = match indexes {
        EntryIndexes::Unique(index) => [Some(index), None, None],
        EntryIndexes::NonUnique(indexes) => *indexes.distinct().indexes(),
    };
    super::imp::PreparedDuplicate::from_indexes(distinct, |index| {
        map.prepare_duplicate(index)
    })
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

/// Mutable references to values matched by a [`TriHashMap`] occupied entry.
pub enum OccupiedEntryMut<
    'a,
    T: TriHashItem,
    S: Clone + BuildHasher = DefaultHashBuilder,
> {
    /// All keys point to the same entry.
    Unique(RefMut<'a, T, S>),
    /// The keys point to different entries, or some keys are not present.
    NonUnique(NonUniqueEntryMut<'a, T, S>),
}

impl<'a, T: TriHashItem + fmt::Debug, S: Clone + BuildHasher> fmt::Debug
    for OccupiedEntryMut<'a, T, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unique(ref_mut) => {
                f.debug_tuple("Unique").field(ref_mut).finish()
            }
            Self::NonUnique(non_unique) => {
                f.debug_tuple("NonUnique").field(non_unique).finish()
            }
        }
    }
}

/// Accessor-backed mutable non-unique entry references.
pub struct NonUniqueEntryMut<
    'a,
    T: TriHashItem,
    S: Clone + BuildHasher = DefaultHashBuilder,
> {
    refs: [Option<RefMut<'a, T, S>>; 3],
    len: usize,
    key_to_slot: [Option<usize>; 3],
}

impl<'a, T: TriHashItem + fmt::Debug, S: Clone + BuildHasher> fmt::Debug
    for NonUniqueEntryMut<'a, T, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NonUniqueEntryMut")
            .field("refs", &self.refs)
            .field("len", &self.len)
            .field("key_to_slot", &self.key_to_slot)
            .finish()
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher> OccupiedEntryMut<'a, T, S> {
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
    /// Returns a mutable reference to the value if the entry is unique.
    #[inline]
    pub fn as_unique(&mut self) -> Option<RefMut<'_, T, S>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::NonUnique(_) => None,
        }
    }
    /// Returns a mutable reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(0)
    }
    /// Returns a mutable reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(1)
    }
    /// Returns a mutable reference to the value fetched by the third key.
    #[inline]
    pub fn by_key3(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(2)
    }
    fn by_key(&mut self, key: usize) -> Option<RefMut<'_, T, S>> {
        match self {
            Self::Unique(v) => Some(v.reborrow()),
            Self::NonUnique(n) => n.by_key(key),
        }
    }
    /// Calls `f` once for each distinct matched value in first-key-hit order.
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(RefMut<'_, T, S>),
    {
        match self {
            Self::Unique(v) => f(v.reborrow()),
            Self::NonUnique(n) => n.for_each(f),
        }
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher> NonUniqueEntryMut<'a, T, S> {
    pub(super) fn new(
        refs: [Option<RefMut<'a, T, S>>; 3],
        len: usize,
        key_to_slot: [Option<usize>; 3],
    ) -> Self {
        Self { refs, len, key_to_slot }
    }
    /// Returns a mutable reference to the value fetched by the first key.
    #[inline]
    pub fn by_key1(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(0)
    }
    /// Returns a mutable reference to the value fetched by the second key.
    #[inline]
    pub fn by_key2(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(1)
    }
    /// Returns a mutable reference to the value fetched by the third key.
    #[inline]
    pub fn by_key3(&mut self) -> Option<RefMut<'_, T, S>> {
        self.by_key(2)
    }
    #[inline]
    fn by_key(&mut self, key: usize) -> Option<RefMut<'_, T, S>> {
        self.key_to_slot[key]
            .and_then(|slot| self.refs[slot].as_mut().map(RefMut::reborrow))
    }
    /// Calls `f` once for each distinct matched value in first-key-hit order.
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(RefMut<'_, T, S>),
    {
        for value in self.refs[..self.len].iter_mut().flatten() {
            f(value.reborrow());
        }
    }
}
