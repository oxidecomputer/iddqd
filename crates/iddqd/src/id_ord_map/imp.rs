use super::{
    Entry, IdOrdItem, IntoIter, Iter, IterMut, OccupiedEntry, RefMut,
    VacantEntry, tables::IdOrdMapTables,
};
use crate::{
    errors::DuplicateItem,
    internal::{ValidateChaos, ValidateCompact, ValidationError},
    support::{borrow::DormantMutRef, item_set::ItemSet},
};
use alloc::collections::BTreeSet;
use core::{borrow::Borrow, fmt, hash::Hash};
use derive_where::derive_where;

/// An ordered map where the keys are part of the values, based on a B-Tree.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in three b-tree maps. This allows for efficient lookups
/// by any of the three keys, while preventing duplicates.
#[derive_where(Default)]
#[derive(Clone)]
pub struct IdOrdMap<T: IdOrdItem> {
    pub(super) items: ItemSet<T>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: IdOrdMapTables,
}

impl<T: IdOrdItem> IdOrdMap<T> {
    /// Creates a new, empty `IdOrdMap`.
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: IdOrdMapTables::new() }
    }

    /// Creates a new `IdOrdMap` with the given capacity.
    ///
    /// The capacity will be used to initialize the underlying hash table.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity(capacity),
            tables: IdOrdMapTables::new(),
        }
    }

    /// Returns the currently allocated capacity of the map.
    pub fn capacity(&self) -> usize {
        // There's no self.tables.capacity.
        self.items.capacity()
    }

    /// Constructs a new `IdOrdMap` from an iterator of values, rejecting
    /// duplicates.
    ///
    /// To overwrite duplicates instead, use [`IdOrdMap::from_iter`].
    pub fn from_iter_unique<I: IntoIterator<Item = T>>(
        iter: I,
    ) -> Result<Self, DuplicateItem<T>> {
        let mut map = IdOrdMap::new();
        for value in iter {
            // It would be nice to use insert_overwrite here, but that would
            // return a `DuplicateItem<T, &T>`, which can only be converted into
            // an owned value if T: Clone. Doing this via the Entry API means we
            // can return a `DuplicateItem<T>` without requiring T to be Clone.
            match map.entry(value.key()) {
                Entry::Occupied(entry) => {
                    let duplicate = entry.remove();
                    return Err(DuplicateItem::__internal_new(
                        value,
                        vec![duplicate],
                    ));
                }
                Entry::Vacant(entry) => {
                    entry.insert_ref(value);
                }
            }
        }

        Ok(map)
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
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.items, &self.tables)
    }

    /// Iterates over the items in the map, allowing for mutation.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T>
    where
        for<'k> T::Key<'k>: Hash,
    {
        IterMut::new(&mut self.items, &self.tables)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[doc(hidden)]
    pub fn validate(
        &self,
        compactness: ValidateCompact,
        chaos: ValidateChaos,
    ) -> Result<(), ValidationError>
    where
        T: fmt::Debug,
    {
        self.items.validate(compactness)?;
        self.tables.validate(self.len(), compactness)?;

        // Check that the indexes are all correct.

        for (&ix, item) in self.items.iter() {
            let key = item.key();
            let ix1 = match chaos {
                ValidateChaos::Yes => {
                    // Fall back to a linear search.
                    self.linear_search_index(&key)
                }
                ValidateChaos::No => {
                    // Use the B-Tree table to find the index.
                    self.find_index(&key)
                }
            };
            let Some(ix1) = ix1 else {
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

    /// Inserts a value into the set, returning an error if any duplicates were
    /// added.
    pub fn insert_unique(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateItem<T, &T>> {
        let _ = self.insert_unique_impl(value)?;
        Ok(())
    }

    /// Inserts a value into the map, removing and returning the conflicting
    /// item, if any.
    pub fn insert_overwrite(&mut self, value: T) -> Option<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove the item corresponding to the key that is already in the map.
        // 2. Add the item to the map.

        let duplicate = self.remove(value.key());

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicate
    }

    /// Returns true if the map contains the given `key`.
    pub fn contains_key<'a, Q>(&'a self, key: &Q) -> bool
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find_index(key).is_some()
    }

    /// Gets a reference to the value associated with the given `key`.
    pub fn get<'a, Q>(&'a self, key: &Q) -> Option<&'a T>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find(key)
    }

    /// Gets a mutable reference to the item associated with the given `key`.
    ///
    /// Due to borrow checker limitations, this always accepts `T::Key` rather
    /// than a borrowed form of it.
    pub fn get_mut<'a>(&'a mut self, key: T::Key<'_>) -> Option<RefMut<'a, T>>
    where
        for<'k> T::Key<'k>: Hash,
    {
        let index = self.find_index(&T::upcast_key(key))?;
        let item = &mut self.items[index];
        let hash = self.tables.make_hash(item);
        Some(RefMut::new(hash, item))
    }

    /// Removes an item from the map by its `key`.
    ///
    /// Due to borrow checker limitations, this always accepts `T::Key` rather
    /// than a borrowed form of it.
    pub fn remove(&mut self, key: T::Key<'_>) -> Option<T> {
        let Some(remove_index) = self.find_index(&T::upcast_key(key)) else {
            // The item was not found.
            return None;
        };

        self.remove_by_index(remove_index)
    }

    /// Retrieves an entry by its `key`.
    pub fn entry<'a>(&'a mut self, key: T::Key<'_>) -> Entry<'a, T> {
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
        Entry::Vacant(
            // SAFETY: `map` is not used after this point.
            unsafe { VacantEntry::new(dormant_map) },
        )
    }

    fn find<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.find_index(k).map(|ix| &self.items[ix])
    }

    fn linear_search_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.items.iter().find_map(|(index, item)| {
            (item.key().borrow() == k).then_some(*index)
        })
    }

    fn find_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        T::Key<'a>: Borrow<Q>,
        T: 'a,
        Q: Ord + ?Sized,
    {
        self.tables.key_to_item.find_index(k, |index| self.items[index].key())
    }

    pub(super) fn get_by_index(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub(super) fn get_by_index_mut(
        &mut self,
        index: usize,
    ) -> Option<RefMut<'_, T>>
    where
        for<'k> T::Key<'k>: Hash,
    {
        let item = self.items.get_mut(index)?;
        let hash = self.tables.make_hash(item);
        Some(RefMut::new(hash, item))
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

        if let Some(index) = self
            .tables
            .key_to_item
            .find_index(&key, |index| self.items[index].key())
        {
            duplicates.insert(index);
        }

        if !duplicates.is_empty() {
            drop(key);
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.next_index();
        self.tables
            .key_to_item
            .insert(next_index, &key, |index| self.items[index].key());
        drop(key);
        self.items.insert_at_next_index(value);

        Ok(next_index)
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the table.
        self.tables.key_to_item.remove(remove_index, value.key(), |index| {
            if index == remove_index {
                value.key()
            } else {
                self.items[index].key()
            }
        });

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

impl<T: IdOrdItem> fmt::Debug for IdOrdMap<T>
where
    T: fmt::Debug,
    for<'k> T::Key<'k>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|item| (item.key(), item)))
            .finish()
    }
}

impl<T: IdOrdItem + PartialEq> PartialEq for IdOrdMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // Items are stored in sorted order, so we can just walk over both
        // iterators.
        if self.items.len() != other.items.len() {
            return false;
        }

        self.iter().zip(other.iter()).all(|(item1, item2)| {
            // Check that the items are equal.
            item1 == item2
        })
    }
}

// The Eq bound on T ensures that the IdOrdMap forms an equivalence class.
impl<T: IdOrdItem + Eq> Eq for IdOrdMap<T> {}

impl<'a, T: IdOrdItem> IntoIterator for &'a IdOrdMap<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: IdOrdItem> IntoIterator for &'a mut IdOrdMap<T>
where
    for<'k> T::Key<'k>: Hash,
{
    type Item = RefMut<'a, T>;
    type IntoIter = IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: IdOrdItem> IntoIterator for IdOrdMap<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items, self.tables)
    }
}

/// The `FromIterator` implementation for `IdOrdMap` overwrites duplicate
/// items.
///
/// To reject duplicates, use [`IdOrdMap::from_iter_unique`].
impl<T: IdOrdItem> FromIterator<T> for IdOrdMap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = IdOrdMap::new();
        for value in iter {
            map.insert_overwrite(value);
        }
        map
    }
}
