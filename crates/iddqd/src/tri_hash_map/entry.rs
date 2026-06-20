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
/// # Differences from single-key entries
///
/// This entry API does not behave exactly like
/// [`std::collections::HashMap::entry`], because three independent key lookups
/// can match zero, one, or several existing items.
///
/// [`VacantEntry`] is returned only when none of `key1`, `key2`, or `key3`
/// passed to [`TriHashMap::entry`] matches an item. [`VacantEntry::insert`] and
/// [`VacantEntry::insert_entry`] insert only after checking that the inserted
/// value's key hashes match the hashes of those three entry keys.
///
/// [`OccupiedEntry`] is returned whenever at least one key matches. It is
/// unique only when all three key positions hit the same item (`A / A / A`).
/// Partial hits (`A / A / None`, `A / None / A`, `None / A / A`) and mixed hits
/// (`A / A / B`, `A / B / A`, `A / B / C`) are occupied non-unique entries, so
/// [`Entry::or_insert`] and [`Entry::or_insert_with`] do not insert for them.
///
/// Non-unique access preserves the per-key mapping. For example, `A / A / B`
/// means key 1 and key 2 both map to `A`, while key 3 maps to `B`. Shared and
/// mutable accessors may therefore return the same item for more than one key
/// position. Mutable non-unique access is accessor-backed: it stores one
/// mutable reference per distinct item and maps key positions to those slots,
/// so cases such as `A / A / B` do not expose aliased mutable references.
/// Mutable accessors take `&mut self` and reborrow sequentially.
///
/// Methods that visit, remove, or replace multiple matched items use
/// deterministic first-key-hit order and deduplicate repeated indexes:
///
/// * `A / A / None`, `A / None / A`, and `None / A / A` visit `[A]`.
/// * `A / A / B` and `A / B / A` visit `[A, B]`.
/// * `A / B / C` visits `[A, B, C]`.
/// * `None / B / A` visits `[B, A]`.
///
/// [`Entry::and_modify`] visits each distinct occupied item once.
/// [`OccupiedEntry::remove`] removes each distinct matched item once.
/// [`OccupiedEntry::insert`] replaces each distinct matched item once, returns
/// the removed items in first-key-hit order, and leaves the occupied entry
/// unique for the replacement item after a successful replacement.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{TriHashItem, TriHashMap, tri_hash_map, tri_upcast};
///
/// #[derive(Debug, PartialEq, Eq)]
/// struct Item {
///     id: u32,
///     name: String,
///     tag: char,
///     value: i32,
/// }
///
/// impl TriHashItem for Item {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///     type K3<'a> = char;
///
///     fn key1(&self) -> Self::K1<'_> {
///         self.id
///     }
///     fn key2(&self) -> Self::K2<'_> {
///         &self.name
///     }
///     fn key3(&self) -> Self::K3<'_> {
///         self.tag
///     }
///     tri_upcast!();
/// }
///
/// let mut map = TriHashMap::new();
/// map.insert_unique(Item {
///     id: 1,
///     name: "foo".to_string(),
///     tag: 'x',
///     value: 10,
/// })
/// .unwrap();
/// map.insert_unique(Item {
///     id: 2,
///     name: "bar".to_string(),
///     tag: 'y',
///     value: 20,
/// })
/// .unwrap();
///
/// // A / A / A: all three keys point to the same item, so the entry is unique.
/// match map.entry(1, "foo", 'x') {
///     tri_hash_map::Entry::Occupied(entry) => {
///         assert!(entry.is_unique());
///         assert_eq!(entry.get().as_unique().unwrap().value, 10);
///     }
///     tri_hash_map::Entry::Vacant(_) => panic!("should be occupied"),
/// }
///
/// // None / None / None: no key is present, so the entry is vacant.
/// map.entry(3, "baz", 'z').or_insert(Item {
///     id: 3,
///     name: "baz".to_string(),
///     tag: 'z',
///     value: 30,
/// });
/// assert_eq!(map.len(), 3);
///
/// // A / A / None: partial hits are occupied non-unique entries.
/// let entry_ref = match map.entry(1, "foo", 'q') {
///     tri_hash_map::Entry::Occupied(entry) => {
///         assert!(entry.is_non_unique());
///         entry.into_ref()
///     }
///     tri_hash_map::Entry::Vacant(_) => panic!("should be occupied"),
/// };
/// assert_eq!(entry_ref.by_key1().unwrap().id, 1);
/// assert_eq!(entry_ref.by_key2().unwrap().id, 1);
/// assert_eq!(entry_ref.by_key3(), None);
///
/// // A / A / B: mixed hits preserve per-key mapping and are not inserted into
/// // by or_insert.
/// let before = map.len();
/// let mut entry_mut = map.entry(1, "foo", 'y').or_insert_with(|| {
///     panic!("occupied non-unique entries do not call the default")
/// });
/// assert_eq!(entry_mut.by_key1().unwrap().id, 1);
/// assert_eq!(entry_mut.by_key2().unwrap().id, 1);
/// assert_eq!(entry_mut.by_key3().unwrap().id, 2);
/// drop(entry_mut);
/// assert_eq!(map.len(), before);
///
/// // Replacement removes each distinct matched item once in first-key-hit
/// // order, then the entry becomes unique for the replacement.
/// match map.entry(1, "foo", 'y') {
///     tri_hash_map::Entry::Occupied(mut entry) => {
///         let removed = entry.insert(Item {
///             id: 1,
///             name: "foo".to_string(),
///             tag: 'y',
///             value: 99,
///         });
///         assert_eq!(
///             removed.iter().map(|item| item.id).collect::<Vec<_>>(),
///             vec![1, 2]
///         );
///         assert!(entry.is_unique());
///     }
///     tri_hash_map::Entry::Vacant(_) => panic!("should be occupied"),
/// }
/// # }
/// ```
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
    /// Provides in-place mutable access to occupied entries before returning
    /// the entry for further chaining.
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

    /// Ensures a value is in the entry by inserting `value` only if vacant,
    /// and returns mutable occupied access to the entry.
    ///
    /// Partial and mixed occupied entries are not vacant, so this method does
    /// not insert for states such as `A / A / None` or `A / A / B`.
    ///
    /// # Panics
    ///
    /// Panics before mutation if `value`'s key hashes differ from the hashes
    /// of the keys passed to [`TriHashMap::entry`].
    #[inline]
    pub fn or_insert(self, value: T) -> OccupiedEntryMut<'a, T, S> {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                OccupiedEntryMut::Unique(entry.insert(value))
            }
        }
    }

    /// Ensures a value is in the entry by inserting the result of `default`
    /// only if vacant, and returns mutable occupied access to the entry.
    ///
    /// `default` is not called for unique, partial, or mixed occupied entries.
    ///
    /// # Panics
    ///
    /// Panics before mutation if the produced value's key hashes differ from
    /// the hashes of the keys passed to [`TriHashMap::entry`].
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
///
/// This is produced by [`TriHashMap::entry`] only when none of the three
/// provided keys match an existing item.
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
    /// Validation is performed before mutation.
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

    /// Sets the entry to a new value, and returns a unique [`OccupiedEntry`].
    ///
    /// Validation is performed before mutation.
    ///
    /// # Panics
    ///
    /// Panics before mutation if any value key hashes differently from the
    /// corresponding key passed to [`TriHashMap::entry`].
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
///
/// An occupied entry exists whenever at least one of the three key lookups
/// matches. It is unique only when all three keys match the same item; partial
/// and mixed hits are non-unique.
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
    ///
    /// Non-unique shared access preserves per-key mapping. Multiple key
    /// positions may return the same item.
    pub fn get(&self) -> OccupiedEntryRef<'_, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.reborrow_shared() };
        map.get_by_entry_index(self.indexes)
    }

    /// Returns mutable references to values that match the provided keys.
    ///
    /// Non-unique mutable access is accessor-backed and reborrows each
    /// distinct item sequentially, preventing aliased mutable references.
    pub fn get_mut(&mut self) -> OccupiedEntryMut<'_, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.reborrow() };
        map.get_by_entry_index_mut(self.indexes)
    }

    /// Converts self into shared references to items that match the provided
    /// keys.
    pub fn into_ref(self) -> OccupiedEntryRef<'a, T> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index(self.indexes)
    }

    /// Converts self into mutable references to items that match the provided
    /// keys.
    pub fn into_mut(self) -> OccupiedEntryMut<'a, T, S> {
        // SAFETY: The safety assumption behind `Self::new` guarantees that the
        // original reference to the map is no longer used at this point, and
        // there is no active temporary reborrow.
        let map = unsafe { self.map.awaken() };
        map.get_by_entry_index_mut(self.indexes)
    }

    /// Removes all distinct values matched by this entry.
    ///
    /// Each distinct matched item is removed once. Returned items are ordered
    /// by first key hit: for example `A / A / B` returns `[A, B]`, while
    /// `None / B / A` returns `[B, A]`.
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
    /// Each distinct matched item is replaced once. Removed items are returned
    /// in first-key-hit order, and after success this entry is unique for the
    /// replacement item.
    ///
    /// # Panics
    ///
    /// Panics before mutation if `value` does not match the entry key hashes
    /// or if its duplicate/index state is incompatible with this entry.
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
        panic!("replacement item keys do not match this occupied entry");
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
///
/// The unique variant means all three keys matched one item. The non-unique
/// variant preserves per-key mapping for partial and mixed matches.
#[derive(Debug)]
pub enum OccupiedEntryRef<'a, T: TriHashItem> {
    /// All keys point to the same entry.
    Unique(&'a T),
    /// The keys point to different entries, or some keys are not present.
    NonUnique(NonUniqueEntryRef<'a, T>),
}

/// Accessor-backed shared non-unique entry references.
///
/// This type stores each distinct matched item once, records which slot each
/// key position matched, and exposes the mapping through accessor methods.
/// `for_each` visits distinct items once in first-key-hit order.
pub struct NonUniqueEntryRef<'a, T: TriHashItem> {
    values: [Option<&'a T>; 3],
    len: usize,
    key_to_slot: [Option<usize>; 3],
}

impl<'a, T: TriHashItem + fmt::Debug> fmt::Debug for NonUniqueEntryRef<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NonUniqueEntryRef")
            .field("by_key1", &self.by_key1())
            .field("by_key2", &self.by_key2())
            .field("by_key3", &self.by_key3())
            .finish()
    }
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
///
/// Mutable non-unique access is accessor-backed. Accessors take `&mut self` and
/// reborrow one distinct item at a time.
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
///
/// This type stores one mutable reference per distinct matched item and maps
/// key positions to those slots. It does not expose public `by_key1`,
/// `by_key2`, or `by_key3` fields, which prevents aliased mutable references
/// in states such as `A / A / B`.
pub struct NonUniqueEntryMut<
    'a,
    T: TriHashItem,
    S: Clone + BuildHasher = DefaultHashBuilder,
> {
    refs: [Option<RefMut<'a, T, S>>; 3],
    len: usize,
    key_to_slot: [Option<usize>; 3],
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher> NonUniqueEntryMut<'a, T, S> {
    #[inline]
    fn fmt_by_key(&self, key: usize) -> Option<&RefMut<'a, T, S>> {
        self.key_to_slot[key].and_then(|slot| self.refs[slot].as_ref())
    }
}

impl<'a, T, S> fmt::Debug for NonUniqueEntryMut<'a, T, S>
where
    T: TriHashItem + fmt::Debug,
    S: Clone + BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NonUniqueEntryMut")
            .field("by_key1", &self.fmt_by_key(0))
            .field("by_key2", &self.fmt_by_key(1))
            .field("by_key3", &self.fmt_by_key(2))
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
