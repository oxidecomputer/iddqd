use super::{
    Entry, IdHashItem, IntoIter, Iter, IterMut, OccupiedEntry, RefMut,
    VacantEntry, tables::IdIndexMapTables,
};
use crate::{
    DefaultHashBuilder,
    errors::DuplicateItem,
    internal::{ValidateCompact, ValidationError},
    support::{
        alloc::{Allocator, Global, global_alloc},
        borrow::DormantMutRef,
        item_set::ItemSet,
        map_hash::MapHash,
        ordered_set::OrderedSet,
    },
};
use alloc::collections::BTreeSet;
use core::{
    fmt,
    hash::{BuildHasher, Hash},
};
use equivalent::Equivalent;
use hashbrown::hash_table;

/// An index map where the key is part of the value, preserving insertion order.
///
/// Similar to [`IdHashMap`], but maintains the order in which items were inserted,
/// like [`IndexMap`].
///
/// The storage mechanism uses an ordered vector of items with a hash table of
/// integer indexes for efficient lookups by key while preserving insertion order.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{IdHashItem, IdIndexMap, id_upcast};
///
/// // Define a struct with a key.
/// #[derive(Debug, PartialEq, Eq, Hash)]
/// struct MyItem {
///     id: String,
///     value: u32,
/// }
///
/// // Implement IdHashItem for the struct.
/// impl IdHashItem for MyItem {
///     // Keys can borrow from the item.
///     type Key<'a> = &'a str;
///
///     fn key(&self) -> Self::Key<'_> {
///         &self.id
///     }
///
///     id_upcast!();
/// }
///
/// // Create an IdIndexMap and insert items.
/// let mut map = IdIndexMap::new();
/// map.insert_unique(MyItem { id: "foo".to_string(), value: 42 }).unwrap();
/// map.insert_unique(MyItem { id: "bar".to_string(), value: 20 }).unwrap();
///
/// // Look up items by their keys.
/// assert_eq!(map.get("foo").unwrap().value, 42);
/// assert_eq!(map.get("bar").unwrap().value, 20);
/// assert!(map.get("baz").is_none());
///
/// // Iteration preserves insertion order
/// let items: Vec<_> = map.iter().collect();
/// assert_eq!(items[0].id, "foo");
/// assert_eq!(items[1].id, "bar");
/// # }
/// ```
///
/// [`IdHashMap`]: crate::IdHashMap
/// [`IndexMap`]: https://docs.rs/indexmap
#[derive(Clone)]
pub struct IdIndexMap<
    T: IdHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    items: OrderedSet<T, A>,
    tables: IdIndexMapTables<S, A>,
}

impl<T: IdHashItem, S: Default, A: Allocator + Default> Default
    for IdIndexMap<T, S, A>
{
    fn default() -> Self {
        Self {
            items: OrderedSet::with_capacity_in(0, A::default()),
            tables: IdIndexMapTables::default(),
        }
    }
}

#[cfg(feature = "default-hasher")]
impl<T: IdHashItem> IdIndexMap<T> {
    /// Creates a new, empty `IdIndexMap`.
    #[inline]
    pub fn new() -> Self {
        Self {
            items: OrderedSet::default(),
            tables: IdIndexMapTables::default(),
        }
    }

    /// Creates a new `IdIndexMap` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(capacity, global_alloc()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                global_alloc(),
            ),
        }
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher> IdIndexMap<T, S> {
    /// Creates a new, empty `IdIndexMap` with the given hasher.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            items: OrderedSet::default(),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                0,
                hasher,
                global_alloc(),
            ),
        }
    }

    /// Creates a new `IdIndexMap` with the given capacity and hasher.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(capacity, global_alloc()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                capacity,
                hasher,
                global_alloc(),
            ),
        }
    }
}

#[cfg(feature = "default-hasher")]
impl<T: IdHashItem, A: Clone + Allocator> IdIndexMap<T, DefaultHashBuilder, A> {
    /// Creates a new empty `IdIndexMap` using the given allocator.
    pub fn new_in(alloc: A) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(0, alloc.clone()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                0,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }

    /// Creates an empty `IdIndexMap` with the specified capacity using the given allocator.
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(capacity, alloc.clone()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Clone + Allocator>
    IdIndexMap<T, S, A>
{
    /// Creates a new, empty `IdIndexMap` with the given hasher and allocator.
    pub fn with_hasher_in(hasher: S, alloc: A) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(0, alloc.clone()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                0, hasher, alloc,
            ),
        }
    }

    /// Creates a new, empty `IdIndexMap` with the given capacity, hasher, and allocator.
    pub fn with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        alloc: A,
    ) -> Self {
        Self {
            items: OrderedSet::with_capacity_in(capacity, alloc.clone()),
            tables: IdIndexMapTables::with_capacity_and_hasher_in(
                capacity, hasher, alloc,
            ),
        }
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> IdIndexMap<T, S, A> {
    #[cfg(feature = "daft")]
    pub(crate) fn hasher(&self) -> &S {
        self.tables.hasher()
    }

    /// Returns the allocator.
    pub fn allocator(&self) -> &A {
        self.items.allocator()
    }

    /// Returns the currently allocated capacity of the map.
    pub fn capacity(&self) -> usize {
        // items and tables.capacity might theoretically diverge: use
        // items.capacity.
        self.items.capacity()
    }

    /// Returns true if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of items in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterates over the items in the map in insertion order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.items)
    }

    /// Iterates over the items in the map, allowing for mutation, in insertion order.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T, S, A> {
        IterMut::new(&self.tables, &mut self.items)
    }

    /// Checks general invariants of the map.
    #[doc(hidden)]
    pub fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError>
    where
        T: fmt::Debug,
    {
        self.items.validate(compactness)?;
        self.tables.validate(self.len(), compactness)?;

        // Check that the indexes are all correct.
        for (&ix, item) in self.items.iter() {
            let key = item.key();
            let Some(ix1) = self.find_index(&key) else {
                return Err(ValidationError::general(format!(
                    "item at index {ix} has no key1 index"
                )));
            };

            if ix1 != ix {
                return Err(ValidationError::General(format!(
                    "item at index {} has mismatched indexes: ix1: {}",
                    ix, ix1,
                )));
            }
        }

        Ok(())
    }

    /// Inserts a value into the map, removing and returning the conflicting item, if any.
    #[doc(alias = "insert")]
    pub fn insert_overwrite(&mut self, value: T) -> Option<T> {
        // TODO: use swap_remove
    }

    /// Inserts a value into the map, returning an error if any duplicates were added.
    pub fn insert_unique(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateItem<T, &T>> {
        let _ = self.insert_unique_impl(value)?;
        Ok(())
    }

    /// Returns true if the map contains the given key.
    pub fn contains_key<'a, Q>(&'a self, key1: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.find_index(key1).is_some()
    }

    /// Gets a reference to the value associated with the given key.
    pub fn get<'a, Q>(&'a self, key: &Q) -> Option<&'a T>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.find_index(key).map(|ix| &self.items[ix])
    }

    /// Gets a mutable reference to the value associated with the given key.
    pub fn get_mut<'a, Q>(&'a mut self, key: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find_index(key)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes = awakened_map.tables.make_hash(item);
        Some(RefMut::new(hashes, item))
    }

    /// Gets the item at the given index.
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    /// Gets a mutable reference to the item at the given index.
    pub fn get_index_mut(&mut self, index: usize) -> Option<RefMut<'_, T, S>> {
        let item = self.items.get_mut(index)?;
        let hashes = self.tables.make_hash(item);
        Some(RefMut::new(hashes, item))
    }

    /// Gets the index of the item with the given key.
    pub fn get_index_of<'a, Q>(&'a self, key: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.find_index(key)
    }

    /// Removes and returns the item at the given index.
    pub fn shift_remove_index(&mut self, index: usize) -> Option<T> {
        let index = self.items.shift_remove(index);
        // Change the index of all items in the hash table greater than the
        // removed index
        self.tables.shift_remove_index(index);
        Some(item)
    }

    /// Removes and returns the item at the given index, swapping it with the last item.
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        // TODO: Implement
        todo!()
    }

    /// Retrieves an entry by its key.
    pub fn entry<'a>(&'a mut self, key: T::Key<'_>) -> Entry<'a, T, S, A> {
        // TODO: Implement
        todo!()
    }

    /// Moves an item from one index to another.
    pub fn move_index(&mut self, from: usize, to: usize) {
        // TODO: Implement
        todo!()
    }

    /// Swaps two items by their indices.
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        // TODO: Implement
        todo!()
    }

    /// Reverses the order of items in the map.
    pub fn reverse(&mut self) {
        // TODO: Implement
        todo!()
    }

    /// Sorts the items in the map by the given comparison function.
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> core::cmp::Ordering,
    {
        // TODO: Implement
        todo!()
    }

    /// Sorts the items in the map by their keys.
    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        // TODO: Implement
        todo!()
    }

    /// Sorts the items in the map by their keys using a cached key function.
    pub fn sort_by_cached_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        // TODO: Implement
        todo!()
    }

    // Internal helper methods
    pub(super) fn get_by_index(&self, index: usize) -> Option<&T> {
        // TODO: Implement
        todo!()
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T, S>> {
        // TODO: Implement
        todo!()
    }

    pub(super) fn insert_unique_impl(
        &mut self,
        value: T,
    ) -> Result<usize, DuplicateItem<T, &T>> {
        // TODO: Implement
        todo!()
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        // TODO: Implement
        todo!()
    }

    pub(super) fn replace_at_index(&mut self, index: usize, value: T) -> T {
        // TODO: Implement
        todo!()
    }
}

impl<T, S: Clone + BuildHasher, A: Allocator> fmt::Debug for IdIndexMap<T, S, A>
where
    T: IdHashItem + fmt::Debug,
    for<'k> T::Key<'k>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem + PartialEq, S: Clone + BuildHasher, A: Allocator> PartialEq
    for IdIndexMap<T, S, A>
{
    fn eq(&self, other: &Self) -> bool {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem + Eq, S: Clone + BuildHasher, A: Allocator> Eq
    for IdIndexMap<T, S, A>
{
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> Extend<T>
    for IdIndexMap<T, S, A>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        // TODO: Implement
        todo!()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a IdIndexMap<T, S, A>
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a mut IdIndexMap<T, S, A>
{
    type Item = RefMut<'a, T, S>;
    type IntoIter = IterMut<'a, T, S, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for IdIndexMap<T, S, A>
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, S: Default + Clone + BuildHasher, A: Allocator + Default>
    FromIterator<T> for IdIndexMap<T, S, A>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        // TODO: Implement
        todo!()
    }
}
