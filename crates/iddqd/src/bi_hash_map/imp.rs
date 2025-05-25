use super::{
    Entry, IntoIter, Iter, IterMut, OccupiedEntry, RefMut, VacantEntry,
    entry::OccupiedEntryRef,
    entry_indexes::{DisjointKeys, EntryIndexes},
    tables::BiHashMapTables,
};
use crate::{
    BiHashItem, DefaultHashBuilder,
    bi_hash_map::entry::OccupiedEntryMut,
    errors::DuplicateItem,
    internal::{ValidateCompact, ValidationError},
    support::{
        alloc::{AllocWrapper, Allocator, Global, global_alloc},
        borrow::DormantMutRef,
        fmt_utils::StrDisplayAsDebug,
        item_set::ItemSet,
    },
};
use alloc::{collections::BTreeSet, vec::Vec};
use core::{
    fmt,
    hash::{BuildHasher, Hash},
};
use derive_where::derive_where;
use equivalent::Equivalent;
use hashbrown::hash_table;

/// A 1:1 (bijective) map for two keys and a value.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in two hash tables. This allows for efficient lookups
/// by either of the two keys and prevents duplicates.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{BiHashMap, BiHashItem, bi_upcast};
///
/// // Define a struct with two keys and a value.
/// #[derive(Debug, PartialEq, Eq)]
/// struct MyItem {
///     id: u32,
///     name: &'static str,
///     value: i32,
/// }
///
/// // Implement BiHashItem for the struct.
/// impl BiHashItem for MyItem {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///
///     fn key1(&self) -> Self::K1<'_> { self.id }
///     fn key2(&self) -> Self::K2<'_> { self.name }
///
///     bi_upcast!();
/// }
///
/// // Create a new BiHashMap and insert items.
/// let mut map = BiHashMap::new();
/// map.insert_unique(MyItem { id: 1, name: "foo", value: 42 }).unwrap();
/// map.insert_unique(MyItem { id: 2, name: "bar", value: 99 }).unwrap();
///
/// // Look up by the first key.
/// assert_eq!(map.get1(&1).unwrap().value, 42);
/// assert_eq!(map.get1(&2).unwrap().value, 99);
/// assert!(map.get1(&3).is_none());
///
/// // Look up by the second key.
/// assert_eq!(map.get2(&"foo").unwrap().value, 42);
/// assert_eq!(map.get2(&"bar").unwrap().value, 99);
/// assert!(map.get2(&"baz").is_none());
/// # }
/// ```
#[derive_where(Default; S: Default, A: Default)]
#[derive(Clone)]
pub struct BiHashMap<
    T: BiHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    pub(super) items: ItemSet<T, A>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: BiHashMapTables<S, A>,
}

#[cfg(feature = "default-hasher")]
impl<T: BiHashItem> BiHashMap<T> {
    /// Creates a new, empty `BiHashMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: BiHashMapTables::default() }
    }

    /// Creates a new `BiHashMap` with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, global_alloc()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                global_alloc(),
            ),
        }
    }
}

impl<T: BiHashItem, S: Clone + BuildHasher> BiHashMap<T, S> {
    /// Creates a new `BiHashMap` with the given hasher.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            items: ItemSet::default(),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                0,
                hasher,
                global_alloc(),
            ),
        }
    }

    /// Creates a new `BiHashMap` with the given capacity and hasher.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, global_alloc()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                capacity,
                hasher,
                global_alloc(),
            ),
        }
    }
}

#[cfg(feature = "default-hasher")]
impl<T: BiHashItem, A: Clone + Allocator> BiHashMap<T, DefaultHashBuilder, A> {
    /// Creates a new empty `BiHashMap` using the given allocator.
    pub fn new_in(alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(0, alloc.clone()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                0,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }

    /// Creates an empty `BiHashMap` with the specified capacity using the given
    /// allocator.
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, alloc.clone()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }
}

impl<T: BiHashItem, S: Clone + BuildHasher, A: Clone + Allocator>
    BiHashMap<T, S, A>
{
    /// Creates a new, empty `BiHashMap` with the given allocator.
    pub fn with_hasher_in(hasher: S, alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(0, alloc.clone()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                0, hasher, alloc,
            ),
        }
    }

    /// Creates a new `BiHashMap` with the given capacity, hasher, and
    /// allocator.
    pub fn with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        alloc: A,
    ) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, alloc.clone()),
            tables: BiHashMapTables::with_capacity_and_hasher_in(
                capacity, hasher, alloc,
            ),
        }
    }
}

impl<T: BiHashItem, S: Clone + BuildHasher, A: Allocator> BiHashMap<T, S, A> {
    /// Returns the hasher.
    #[cfg(feature = "daft")]
    #[inline]
    pub(crate) fn hasher(&self) -> &S {
        self.tables.hasher()
    }

    /// Returns the allocator.
    #[inline]
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
    pub fn iter_mut(&mut self) -> IterMut<'_, T, S, A> {
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
            let key1 = item.key1();
            let key2 = item.key2();

            let Some(ix1) = self.find1_index(&key1) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key1 index",
                    ix
                )));
            };
            let Some(ix2) = self.find2_index(&key2) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key2 index",
                    ix
                )));
            };

            if ix1 != ix || ix2 != ix {
                return Err(ValidationError::general(format!(
                    "item at index {} has inconsistent indexes: {}/{}",
                    ix, ix1, ix2
                )));
            }
        }

        Ok(())
    }

    /// Inserts a value into the map, removing any conflicting items and
    /// returning a list of those items.
    #[doc(alias = "insert")]
    pub fn insert_overwrite(&mut self, value: T) -> Vec<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove items corresponding to keys that are already in the map.
        // 2. Add the item to the map.

        let mut duplicates = Vec::new();
        duplicates.extend(self.remove1(&value.key1()));
        duplicates.extend(self.remove2(&value.key2()));

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicates
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

    /// Returns true if the map contains a single item that matches both `key1` and `key2`.
    pub fn contains_key_unique<'a, Q1, Q2>(
        &'a self,
        key1: &Q1,
        key2: &Q2,
    ) -> bool
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.get_unique(key1, key2).is_some()
    }

    /// Gets a reference to the unique item associated with the given `key1` and
    /// `key2`, if it exists.
    pub fn get_unique<'a, Q1, Q2>(
        &'a self,
        key1: &Q1,
        key2: &Q2,
    ) -> Option<&'a T>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let index = self.find1_index(key1)?;
        let item = &self.items[index];
        if key2.equivalent(&item.key2()) { Some(item) } else { None }
    }

    /// Gets a mutable reference to the unique item associated with the given
    /// `key1` and `key2`, if it exists.
    pub fn get_mut_unique<'a, Q1, Q2>(
        &'a mut self,
        key1: &Q1,
        key2: &Q2,
    ) -> Option<RefMut<'a, T, S>>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find1_index(key1)?;
            // Check key2 match before proceeding
            if !key2.equivalent(&map.items[index].key2()) {
                return None;
            }
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes =
            awakened_map.tables.make_hashes::<T>(&item.key1(), &item.key2());
        Some(RefMut::new(hashes, item))
    }

    /// Removes the item uniquely identified by `key1` and `key2`, if it exists.
    pub fn remove_unique<'a, Q1, Q2>(
        &'a mut self,
        key1: &Q1,
        key2: &Q2,
    ) -> Option<T>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find1_index(key1)?;
            if !key2.equivalent(&map.items[remove_index].key2()) {
                return None;
            }
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Returns true if the map contains the given `key1`.
    pub fn contains_key1<'a, Q>(&'a self, key1: &Q) -> bool
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1_index(key1).is_some()
    }

    /// Gets a reference to the value associated with the given `key1`.
    pub fn get1<'a, Q>(&'a self, key1: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1(key1)
    }

    /// Gets a mutable reference to the value associated with the given `key1`.
    pub fn get1_mut<'a, Q>(&'a mut self, key1: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find1_index(key1)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes =
            awakened_map.tables.make_hashes::<T>(&item.key1(), &item.key2());
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key1`.
    pub fn remove1<'a, Q>(&'a mut self, key1: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find1_index(key1)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Returns true if the map contains the given `key2`.
    pub fn contains_key2<'a, Q>(&'a self, key2: &Q) -> bool
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2_index(key2).is_some()
    }

    /// Gets a reference to the value associated with the given `key2`.
    pub fn get2<'a, Q>(&'a self, key2: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2(key2)
    }

    /// Gets a mutable reference to the value associated with the given `key2`.
    pub fn get2_mut<'a, Q>(&'a mut self, key2: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find2_index(key2)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes =
            awakened_map.tables.make_hashes::<T>(&item.key1(), &item.key2());
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key2`.
    pub fn remove2<'a, Q>(&'a mut self, key2: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find2_index(key2)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Retrieves an entry by its keys.
    pub fn entry<'a>(
        &'a mut self,
        key1: T::K1<'_>,
        key2: T::K2<'_>,
    ) -> Entry<'a, T, S, A> {
        let (map, dormant_map) = DormantMutRef::new(self);
        let key1 = T::upcast_key1(key1);
        let key2 = T::upcast_key2(key2);
        let (index1, index2) = {
            // index1 and index2 are explicitly typed to show that it has a
            // trivial Drop impl that doesn't capture anything from map.
            let index1: Option<usize> = map
                .tables
                .k1_to_item
                .find_index(&key1, |index| map.items[index].key1());
            let index2: Option<usize> = map
                .tables
                .k2_to_item
                .find_index(&key2, |index| map.items[index].key2());
            (index1, index2)
        };

        match (index1, index2) {
            (Some(index1), Some(index2)) if index1 == index2 => {
                // The item is already in the map.
                drop(key1);
                Entry::Occupied(
                    // SAFETY: `map` is not used after this point.
                    unsafe {
                        OccupiedEntry::new(
                            dormant_map,
                            EntryIndexes::Unique(index1),
                        )
                    },
                )
            }
            (None, None) => {
                let hashes = map.tables.make_hashes::<T>(&key1, &key2);
                Entry::Vacant(
                    // SAFETY: `map` is not used after this point.
                    unsafe { VacantEntry::new(dormant_map, hashes) },
                )
            }
            (index1, index2) => Entry::Occupied(unsafe {
                OccupiedEntry::new(
                    dormant_map,
                    EntryIndexes::NonUnique { index1, index2 },
                )
            }),
        }
    }

    fn find1<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1_index(k).map(|ix| &self.items[ix])
    }

    fn find1_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.tables.k1_to_item.find_index(k, |index| self.items[index].key1())
    }

    fn find2<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2_index(k).map(|ix| &self.items[ix])
    }

    fn find2_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.tables.k2_to_item.find_index(k, |index| self.items[index].key2())
    }

    pub(super) fn get_by_entry_index(
        &self,
        indexes: EntryIndexes,
    ) -> OccupiedEntryRef<'_, T> {
        match indexes {
            EntryIndexes::Unique(index) => OccupiedEntryRef::Unique(
                self.items.get(index).expect("index is valid"),
            ),
            EntryIndexes::NonUnique { index1, index2 } => {
                let by_key1 = index1
                    .map(|k| self.items.get(k).expect("key1 index is valid"));
                let by_key2 = index2
                    .map(|k| self.items.get(k).expect("key2 index is valid"));
                OccupiedEntryRef::NonUnique { by_key1, by_key2 }
            }
        }
    }

    pub(super) fn get_by_entry_index_mut(
        &mut self,
        indexes: EntryIndexes,
    ) -> OccupiedEntryMut<'_, T, S> {
        match indexes.disjoint_keys() {
            DisjointKeys::Unique(index) => {
                let item = self.items.get_mut(index).expect("index is valid");
                let hashes =
                    self.tables.make_hashes::<T>(&item.key1(), &item.key2());
                OccupiedEntryMut::Unique(RefMut::new(hashes, item))
            }
            DisjointKeys::Key1(index1) => {
                let item =
                    self.items.get_mut(index1).expect("key1 index is valid");
                let hashes =
                    self.tables.make_hashes::<T>(&item.key1(), &item.key2());
                OccupiedEntryMut::NonUnique {
                    by_key1: Some(RefMut::new(hashes, item)),
                    by_key2: None,
                }
            }
            DisjointKeys::Key2(index2) => {
                let item =
                    self.items.get_mut(index2).expect("key2 index is valid");
                let hashes =
                    self.tables.make_hashes::<T>(&item.key1(), &item.key2());
                OccupiedEntryMut::NonUnique {
                    by_key1: None,
                    by_key2: Some(RefMut::new(hashes, item)),
                }
            }
            DisjointKeys::Key12(indexes) => {
                let mut items = self.items.get_disjoint_mut(indexes);
                let item1 = items[0].take().expect("key1 index is valid");
                let item2 = items[1].take().expect("key2 index is valid");
                let hashes1 =
                    self.tables.make_hashes::<T>(&item1.key1(), &item1.key2());
                let hashes2 =
                    self.tables.make_hashes::<T>(&item2.key1(), &item2.key2());

                OccupiedEntryMut::NonUnique {
                    by_key1: Some(RefMut::new(hashes1, item1)),
                    by_key2: Some(RefMut::new(hashes2, item2)),
                }
            }
        }
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T, S>> {
        let borrowed = self.items.get_mut(index)?;
        let hashes =
            self.tables.make_hashes::<T>(&borrowed.key1(), &borrowed.key2());
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
        let (e1, e2) = {
            let k1 = value.key1();
            let k2 = value.key2();

            let e1 = detect_dup_or_insert(
                self.tables
                    .k1_to_item
                    .entry(k1, |index| self.items[index].key1()),
                &mut duplicates,
            );
            let e2 = detect_dup_or_insert(
                self.tables
                    .k2_to_item
                    .entry(k2, |index| self.items[index].key2()),
                &mut duplicates,
            );
            (e1, e2)
        };

        if !duplicates.is_empty() {
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.insert_at_next_index(value);
        // e1 and e2 are all Some because if they were None, duplicates
        // would be non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);

        Ok(next_index)
    }

    pub(super) fn remove_by_entry_index(
        &mut self,
        indexes: EntryIndexes,
    ) -> Vec<T> {
        match indexes {
            EntryIndexes::Unique(index) => {
                // Since all keys match, we can simply replace the item.
                let old_item =
                    self.remove_by_index(index).expect("index is valid");
                vec![old_item]
            }
            EntryIndexes::NonUnique { index1, index2 } => {
                let mut old_items = Vec::new();
                if let Some(index1) = index1 {
                    old_items.push(
                        self.remove_by_index(index1).expect("index1 is valid"),
                    );
                }
                if let Some(index2) = index2 {
                    old_items.push(
                        self.remove_by_index(index2).expect("index2 is valid"),
                    );
                }

                old_items
            }
        }
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the tables.
        let Ok(item1) =
            self.tables.k1_to_item.find_entry(&value.key1(), |index| {
                if index == remove_index {
                    value.key1()
                } else {
                    self.items[index].key1()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k1_to_item");
        };
        let Ok(item2) =
            self.tables.k2_to_item.find_entry(&value.key2(), |index| {
                if index == remove_index {
                    value.key2()
                } else {
                    self.items[index].key2()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k2_to_item")
        };

        item1.remove();
        item2.remove();

        Some(value)
    }

    pub(super) fn replace_at_indexes(
        &mut self,
        indexes: EntryIndexes,
        value: T,
    ) -> (usize, Vec<T>) {
        match indexes {
            EntryIndexes::Unique(index) => {
                let old_item = &self.items[index];
                if old_item.key1() != value.key1() {
                    panic!("key1 mismatch");
                }
                if old_item.key2() != value.key2() {
                    panic!("key2 mismatch");
                }

                // Since all keys match, we can simply replace the item.
                let old_item = self.items.replace(index, value);
                (index, vec![old_item])
            }
            EntryIndexes::NonUnique { index1, index2 } => {
                let mut old_items = Vec::new();
                if let Some(index1) = index1 {
                    let old_item = &self.items[index1];
                    if old_item.key1() != value.key1() {
                        panic!("key1 mismatch");
                    }
                    old_items.push(self.remove_by_index(index1).unwrap());
                }
                if let Some(index2) = index2 {
                    let old_item = &self.items[index2];
                    if old_item.key2() != value.key2() {
                        panic!("key2 mismatch");
                    }
                    old_items.push(self.remove_by_index(index2).unwrap());
                }

                // Insert the new item.
                let Ok(next_index) = self.insert_unique_impl(value) else {
                    unreachable!(
                        "insert_unique cannot fail after removing duplicates"
                    );
                };
                (next_index, old_items)
            }
        }
    }
}

impl<T, S, A> fmt::Debug for BiHashMap<T, S, A>
where
    T: BiHashItem + fmt::Debug,
    for<'k> T::K1<'k>: fmt::Debug,
    for<'k> T::K2<'k>: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct KeyMap<'a, T: BiHashItem + 'a> {
            key1: T::K1<'a>,
            key2: T::K2<'a>,
        }

        impl<'a, T: BiHashItem> fmt::Debug for KeyMap<'a, T>
        where
            for<'k> T::K1<'k>: fmt::Debug,
            for<'k> T::K2<'k>: fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // We don't want to show key1 and key2 as a tuple since it's
                // misleading (suggests maps of tuples). The best we can do
                // instead is to show "{k1: "abc", k2: "xyz"}"
                f.debug_map()
                    .entry(&StrDisplayAsDebug("k1"), &self.key1)
                    .entry(&StrDisplayAsDebug("k2"), &self.key2)
                    .finish()
            }
        }

        f.debug_map()
            .entries(self.items.iter().map(|(_, item)| {
                (KeyMap::<T> { key1: item.key1(), key2: item.key2() }, item)
            }))
            .finish()
    }
}

impl<T: BiHashItem + PartialEq, S: Clone + BuildHasher, A: Allocator> PartialEq
    for BiHashMap<T, S, A>
{
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for BiHashMap is tricky because BiHashMap is
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
            let k1 = item.key1();
            let k2 = item.key2();

            // Check that the indexes are the same in the other map.
            let Some(other_ix1) = other.find1_index(&k1) else {
                return false;
            };
            let Some(other_ix2) = other.find2_index(&k2) else {
                return false;
            };

            if other_ix1 != other_ix2 {
                // All the keys were present but they didn't point to the same
                // item.
                return false;
            }

            // Check that the other map's item is the same as this map's
            // item. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix1 and other_ix2 are
            // Some, we know that it is valid and points to the expected item.
            let other_item = &other.items[other_ix1];
            if item != other_item {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the BiHashMap forms an equivalence class.
impl<T: BiHashItem + Eq, S: Clone + BuildHasher, A: Allocator> Eq
    for BiHashMap<T, S, A>
{
}

fn detect_dup_or_insert<'a, A: Allocator>(
    item: hash_table::Entry<'a, usize, AllocWrapper<A>>,
    duplicates: &mut BTreeSet<usize>,
) -> Option<hash_table::VacantEntry<'a, usize, AllocWrapper<A>>> {
    match item {
        hash_table::Entry::Vacant(slot) => Some(slot),
        hash_table::Entry::Occupied(slot) => {
            duplicates.insert(*slot.get());
            None
        }
    }
}

/// The `Extend` implementation overwrites duplicates. In the future, there will
/// also be an `extend_unique` method that will return an error.
impl<T: BiHashItem, S: Clone + BuildHasher, A: Allocator> Extend<T>
    for BiHashMap<T, S, A>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert_overwrite(item);
        }
    }
}

impl<'a, T: BiHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a BiHashMap<T, S, A>
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: BiHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a mut BiHashMap<T, S, A>
{
    type Item = RefMut<'a, T, S>;
    type IntoIter = IterMut<'a, T, S, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: BiHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for BiHashMap<T, S, A>
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items)
    }
}

/// The `FromIterator` implementation for `BiHashMap` overwrites duplicate
/// items.
impl<T: BiHashItem, S: Clone + BuildHasher + Default> FromIterator<T>
    for BiHashMap<T, S>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = BiHashMap::default();
        for item in iter {
            map.insert_overwrite(item);
        }
        map
    }
}
