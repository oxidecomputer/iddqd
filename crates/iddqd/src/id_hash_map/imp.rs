use super::{
    Entry, IdHashItem, IntoIter, Iter, IterMut, OccupiedEntry, RefMut,
    VacantEntry, tables::IdHashMapTables,
};
use crate::{
    DefaultHashBuilder,
    errors::DuplicateItem,
    internal::{ValidateCompact, ValidationError},
    support::{borrow::DormantMutRef, item_set::ItemSet, map_hash::MapHash},
};
use alloc::collections::BTreeSet;
use core::{
    fmt,
    hash::{BuildHasher, Hash},
};
use derive_where::derive_where;
use equivalent::Equivalent;
use hashbrown::hash_table;

/// A hash map where the key is part of the value.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in a hash table. This allows for efficient lookups by
/// the key and prevents duplicates.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{IdHashMap, IdHashItem, id_upcast};
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
/// // Create an IdHashMap and insert items.
/// let mut map = IdHashMap::new();
/// map.insert_unique(MyItem { id: "foo".to_string(), value: 42 }).unwrap();
/// map.insert_unique(MyItem { id: "bar".to_string(), value: 20 }).unwrap();
///
/// // Look up items by their keys.
/// assert_eq!(map.get("foo").unwrap().value, 42);
/// assert_eq!(map.get("bar").unwrap().value, 20);
/// assert!(map.get("baz").is_none());
/// # }
/// ```
#[derive_where(Default; S: Default)]
#[derive(Clone)]
pub struct IdHashMap<T: IdHashItem, S = DefaultHashBuilder> {
    pub(super) items: ItemSet<T>,
    tables: IdHashMapTables<S>,
}

#[cfg(feature = "default-hasher")]
impl<T: IdHashItem> IdHashMap<T> {
    /// Creates a new, empty `IdHashMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: IdHashMapTables::default() }
    }

    /// Creates a new `IdHashMap` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity(capacity),
            tables: IdHashMapTables::with_capacity_and_hasher(
                capacity,
                DefaultHashBuilder::default(),
            ),
        }
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher> IdHashMap<T, S> {
    /// Creates a new, empty `IdHashMap` with the given hasher.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            items: ItemSet::default(),
            tables: IdHashMapTables::with_capacity_and_hasher(0, hasher),
        }
    }

    /// Creates a new `IdHashMap` with the given capacity and hasher.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            items: ItemSet::with_capacity(capacity),
            tables: IdHashMapTables::with_capacity_and_hasher(capacity, hasher),
        }
    }

    #[cfg(feature = "daft")]
    pub(crate) fn hasher(&self) -> &S {
        self.tables.hasher()
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

    /// Iterates over the items in the map.
    ///
    /// Similar to [`HashMap`], the iteration order is arbitrary and not
    /// guaranteed to be stable.
    ///
    /// [`HashMap`]: std::collections::HashMap
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.items)
    }

    /// Iterates over the items in the map, allowing for mutation.
    ///
    /// Similar to [`HashMap`], the iteration order is arbitrary and not
    /// guaranteed to be stable.
    ///
    /// [`HashMap`]: std::collections::HashMap
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T, S> {
        IterMut::new(&self.tables, &mut self.items)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[doc(hidden)]
    pub fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError>
    where
        T: core::fmt::Debug,
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

    /// Inserts a value into the map, removing and returning the conflicting
    /// item, if any.
    #[doc(alias = "insert")]
    pub fn insert_overwrite(&mut self, value: T) -> Option<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove items corresponding to the key that is already in the map.
        // 2. Add the item to the map.

        let duplicate = self.remove(&value.key());

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicate
    }

    /// Inserts a value into the set, returning an error if any duplicates were
    /// added.
    pub fn insert_unique(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateItem<T, &T>> {
        let _ = self.insert_unique_impl(value)?;
        Ok(())
    }

    /// Returns true if the map contains the given `key1`.
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

    /// Gets a mutable reference to the value associated with the given `key`.
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

    /// Removes an item from the map by its `key`.
    pub fn remove<'a, Q>(&'a mut self, key: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find_index(key)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        let value = awakened_map
            .items
            .remove(remove_index)
            .expect("items missing key1 that was just retrieved");

        // Remove the value from the tables.
        let Ok(item1) =
            awakened_map.tables.key_to_item.find_entry(&value.key(), |index| {
                if index == remove_index {
                    value.key()
                } else {
                    awakened_map.items[index].key()
                }
            })
        else {
            // The item was not found.
            panic!("we just looked this item up");
        };

        item1.remove();

        Some(value)
    }

    /// Retrieves an entry by its `key`.
    pub fn entry<'a>(&'a mut self, key: T::Key<'_>) -> Entry<'a, T, S> {
        let (map, dormant_map) = DormantMutRef::new(self);
        let key = T::upcast_key(key);
        {
            // index is explicitly typed to show that it has a trivial Drop impl
            // that doesn't capture anything from map.
            let index: Option<usize> = map
                .tables
                .key_to_item
                .find_index(&key, |index| map.items[index].key());
            if let Some(index) = index {
                drop(key);
                return Entry::Occupied(
                    // SAFETY: `map` is not used after this point.
                    unsafe { OccupiedEntry::new(dormant_map, index) },
                );
            }
        }
        let hash = map.make_key_hash(&key);
        Entry::Vacant(
            // SAFETY: `map` is not used after this point.
            unsafe { VacantEntry::new(dormant_map, hash) },
        )
    }

    fn find_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::Key<'a>> + ?Sized,
    {
        self.tables.key_to_item.find_index(k, |index| self.items[index].key())
    }

    fn make_hash(&self, item: &T) -> MapHash<S> {
        self.tables.make_hash(item)
    }

    fn make_key_hash(&self, key: &T::Key<'_>) -> MapHash<S> {
        self.tables.make_key_hash::<T>(key)
    }

    pub(super) fn get_by_index(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T, S>> {
        let hashes = self.make_hash(&self.items[index]);
        let item = &mut self.items[index];
        Some(RefMut::new(hashes, item))
    }

    pub(super) fn insert_unique_impl(
        &mut self,
        value: T,
    ) -> Result<usize, DuplicateItem<T, &T>> {
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new item, because we
        // don't want to partially insert the new item and then have to roll
        // back.
        let key = value.key();

        let entry = match self
            .tables
            .key_to_item
            .entry(key, |index| self.items[index].key())
        {
            hash_table::Entry::Occupied(slot) => {
                duplicates.insert(*slot.get());
                None
            }
            hash_table::Entry::Vacant(slot) => Some(slot),
        };

        if !duplicates.is_empty() {
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.insert_at_next_index(value);
        entry.unwrap().insert(next_index);

        Ok(next_index)
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the tables.
        let Ok(item) =
            self.tables.key_to_item.find_entry(&value.key(), |index| {
                if index == remove_index {
                    value.key()
                } else {
                    self.items[index].key()
                }
            })
        else {
            // The item was not found.
            panic!("we just looked this item up");
        };

        item.remove();

        Some(value)
    }

    pub(super) fn replace_at_index(&mut self, index: usize, value: T) -> T {
        // We check the key before removing it, to avoid leaving the map in an
        // inconsistent state.
        let old_key =
            self.get_by_index(index).expect("index is known to be valid").key();
        if T::upcast_key(old_key) != value.key() {
            panic!(
                "must insert a value with \
                 the same key used to create the entry"
            );
        }

        // Now that we know the key is the same, we can replace the value
        // directly without needing to tweak any tables.
        self.items.replace(index, value)
    }
}

impl<T, S: Clone + BuildHasher> fmt::Debug for IdHashMap<T, S>
where
    T: IdHashItem + fmt::Debug,
    for<'k> T::Key<'k>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|item| (item.key(), item)))
            .finish()
    }
}

impl<T: IdHashItem + PartialEq, S: Clone + BuildHasher> PartialEq
    for IdHashMap<T, S>
{
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for IdHashMap is tricky because IdHashMap is
        // not semantically like an IndexMap: two maps are equivalent even if
        // their items are in a different order. In other words, any permutation
        // of items is equivalent.
        //
        // We also can't sort the items because they're not necessarily Ord.
        //
        // So we write a custom equality check that checks that each key in one
        // map points to the same item as in the other map.

        if self.items.len() != other.items.len() {
            return false;
        }

        // Walk over all the items in the first map and check that they point to
        // the same item in the second map.
        for item in self.items.values() {
            let k1 = item.key();

            // Check that the indexes are the same in the other map.
            let Some(other_ix) = other.find_index(&k1) else {
                return false;
            };

            // Check that the other map's item is the same as this map's
            // item. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix is Some, we know that it is
            // valid and points to the expected item.
            let other_item = &other.items[other_ix];
            if item != other_item {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the TriHashMap forms an equivalence class.
impl<T: IdHashItem + Eq, S: Clone + BuildHasher> Eq for IdHashMap<T, S> {}

/// The `Extend` implementation overwrites duplicates. In the future, there will
/// also be an `extend_unique` method that will return an error.
impl<T: IdHashItem, S: Clone + BuildHasher> Extend<T> for IdHashMap<T, S> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert_overwrite(item);
        }
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher> IntoIterator
    for &'a IdHashMap<T, S>
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher> IntoIterator
    for &'a mut IdHashMap<T, S>
{
    type Item = RefMut<'a, T, S>;
    type IntoIter = IterMut<'a, T, S>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher> IntoIterator for IdHashMap<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items)
    }
}

/// The `FromIterator` implementation for `IdHashMap` overwrites duplicate
/// items.
impl<T: IdHashItem, S: Default + Clone + BuildHasher> FromIterator<T>
    for IdHashMap<T, S>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = IdHashMap::default();
        for item in iter {
            map.insert_overwrite(item);
        }
        map
    }
}
