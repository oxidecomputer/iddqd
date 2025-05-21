//! A "table" of b-tree-based indexes.
//!
//! Similar to [`super::hash_table::MapHashTable`], b-tree based tables store
//! integers (that are indexes corresponding to items), but use an external
//! comparator.

use super::map_hash::MapHash;
use crate::internal::{TableValidationError, ValidateCompact};
use std::{
    borrow::Borrow,
    cell::Cell,
    cmp::Ordering,
    collections::{btree_set, BTreeSet},
    hash::{BuildHasher, Hash, RandomState},
    marker::PhantomData,
};

thread_local! {
    static CMP: Cell<Option<&'static dyn Fn(Index, Index) -> Ordering>>
        = const { Cell::new(None) };
}

/// A B-tree-based table with an external comparator.
#[derive(Clone, Debug, Default)]
pub(crate) struct MapBTreeTable {
    items: BTreeSet<Index>,
    hash_state: RandomState,
}

impl MapBTreeTable {
    #[doc(hidden)]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[doc(hidden)]
    pub(crate) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), TableValidationError> {
        if self.len() != expected_len {
            return Err(TableValidationError::new(format!(
                "expected length {expected_len}, was {}",
                self.len(),
            )));
        }

        match compactness {
            ValidateCompact::Compact => {
                // All items between 0 (inclusive) and self.len() (exclusive)
                // are present, and there are no duplicates. Also, the sentinel
                // value should not be stored.
                let mut indexes: Vec<_> = Vec::with_capacity(expected_len);
                for index in &self.items {
                    match index.0 {
                        Index::SENTINEL_VALUE => {
                            return Err(TableValidationError::new(
                                "sentinel value should not be stored in map",
                            ));
                        }
                        v => {
                            indexes.push(v);
                        }
                    }
                }
                indexes.sort_unstable();
                for (i, index) in indexes.iter().enumerate() {
                    if *index != i {
                        return Err(TableValidationError::new(format!(
                            "value at index {i} should be {i}, was {index}",
                        )));
                    }
                }
            }
            ValidateCompact::NonCompact => {
                // There should be no duplicates, and the sentinel value
                // should not be stored.
                let indexes: Vec<_> = self.items.iter().copied().collect();
                let index_set: BTreeSet<usize> =
                    indexes.iter().map(|ix| ix.0).collect();
                if index_set.len() != indexes.len() {
                    return Err(TableValidationError::new(format!(
                        "expected no duplicates, but found {} duplicates \
                         (values: {:?})",
                        indexes.len() - index_set.len(),
                        indexes,
                    )));
                }
                if index_set.contains(&Index::SENTINEL_VALUE) {
                    return Err(TableValidationError::new(
                        "sentinel value should not be stored in map",
                    ));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn find_index<K, Q, F>(
        &self,
        key: &Q,
        lookup: F,
    ) -> Option<usize>
    where
        F: Fn(usize) -> K,
        K: Ord + Borrow<Q>,
        Q: ?Sized + Ord,
    {
        let f = find_cmp(key, lookup);

        let guard = CmpDropGuard::new(&f);

        let ret = match self.items.get(&Index::SENTINEL) {
            Some(Index(v)) if *v == Index::SENTINEL_VALUE => {
                panic!("internal map shouldn't store sentinel value")
            }
            Some(Index(v)) => Some(*v),
            None => {
                // The key is not in the table.
                None
            }
        };

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
        ret
    }

    pub(crate) fn insert<K, Q, F>(&mut self, index: usize, key: &Q, lookup: F)
    where
        F: Fn(usize) -> K,
        K: Ord + Borrow<Q>,
        Q: ?Sized + Ord,
    {
        let f = insert_cmp(index, key, lookup);
        let guard = CmpDropGuard::new(&f);

        self.items.insert(Index::new(index));

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
    }

    pub(crate) fn remove<K, F>(&mut self, index: usize, key: K, lookup: F)
    where
        F: Fn(usize) -> K,
        K: Ord,
    {
        let f = insert_cmp(index, &key, lookup);
        let guard = CmpDropGuard::new(&f);

        self.items.remove(&Index::new(index));

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
    }

    pub(crate) fn iter(&self) -> Iter {
        Iter::new(self.items.iter())
    }

    pub(crate) fn into_iter(self) -> IntoIter {
        IntoIter::new(self.items.into_iter())
    }

    pub(crate) fn compute_hash<K: Hash>(&self, key: K) -> MapHash {
        MapHash {
            state: self.hash_state.clone(),
            hash: self.hash_state.hash_one(key),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Iter<'a> {
    inner: btree_set::Iter<'a, Index>,
}

impl<'a> Iter<'a> {
    fn new(inner: btree_set::Iter<'a, Index>) -> Self {
        Self { inner }
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|index| index.0)
    }
}

#[derive(Debug)]
pub(crate) struct IntoIter {
    inner: btree_set::IntoIter<Index>,
}

impl IntoIter {
    fn new(inner: btree_set::IntoIter<Index>) -> Self {
        Self { inner }
    }
}

impl Iterator for IntoIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|index| index.0)
    }
}

fn find_cmp<'a, K, Q, F>(
    key: &'a Q,
    lookup: F,
) -> impl Fn(Index, Index) -> Ordering + 'a
where
    F: Fn(usize) -> K + 'a,
    K: Ord + Borrow<Q> + 'a,
    Q: ?Sized + Ord,
{
    move |a: Index, b: Index| {
        if a.0 == b.0 {
            // This is potentially load-bearing! It means that even if the Eq
            // implementation on map items is wrong, we treat items at the same
            // index as equal.
            //
            // Unsafe code relies on this to ensure that we don't return
            // multiple mutable references to the same index.
            return Ordering::Equal;
        }
        match (a.0, b.0) {
            (Index::SENTINEL_VALUE, v) => key.borrow().cmp(lookup(v).borrow()),
            (v, Index::SENTINEL_VALUE) => lookup(v).borrow().cmp(key.borrow()),
            (a, b) => lookup(a).borrow().cmp(lookup(b).borrow()),
        }
    }
}

fn insert_cmp<'a, K, Q, F>(
    index: usize,
    key: &'a Q,
    lookup: F,
) -> impl Fn(Index, Index) -> Ordering + 'a
where
    F: Fn(usize) -> K + 'a,
    K: Ord + Borrow<Q> + 'a,
    Q: ?Sized + Ord,
{
    move |a: Index, b: Index| {
        if a.0 == b.0 {
            // This is potentially load-bearing! It means that even if the Eq
            // implementation on map items is wrong, we treat items at the same
            // index as equal.
            //
            // Unsafe code relies on this to ensure that we don't return
            // multiple mutable references to the same index.
            return Ordering::Equal;
        }
        match (a.0, b.0) {
            // The sentinel value should not be invoked at all, because it's not
            // passed in during insert and not stored in the table.
            (Index::SENTINEL_VALUE, _) | (_, Index::SENTINEL_VALUE) => {
                panic!("sentinel value should not be invoked in insert path")
            }
            (a, b) if a == index => key.borrow().cmp(lookup(b).borrow()),
            (a, b) if b == index => lookup(a).borrow().cmp(key.borrow()),
            (a, b) => lookup(a).borrow().cmp(lookup(b).borrow()),
        }
    }
}

struct CmpDropGuard<'a> {
    _marker: PhantomData<&'a ()>,
}

impl<'a> CmpDropGuard<'a> {
    fn new(f: &'a dyn Fn(Index, Index) -> Ordering) -> Self {
        // CMP lasts only as long as this function and is immediately reset to
        // None once this scope is left.
        let ret = Self { _marker: PhantomData };

        let as_static = unsafe {
            // SAFETY: This is safe because we are not storing the reference
            // anywhere, and it is only used for the lifetime of this
            // CmpDropGuard.
            std::mem::transmute::<
                &'a dyn Fn(Index, Index) -> Ordering,
                &'static dyn Fn(Index, Index) -> Ordering,
            >(f)
        };
        CMP.set(Some(as_static));

        ret
    }
}

impl Drop for CmpDropGuard<'_> {
    fn drop(&mut self) {
        CMP.set(None);
    }
}

#[derive(Clone, Copy, Debug)]
struct Index(usize);

impl Index {
    const SENTINEL_VALUE: usize = usize::MAX;
    const SENTINEL: Self = Self(Self::SENTINEL_VALUE);

    #[inline]
    fn new(value: usize) -> Self {
        if value == Self::SENTINEL_VALUE {
            panic!("btree map overflow, index with value {value:?} was added")
        }
        Self(value)
    }
}

impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        // For non-sentinel indexes, two values are the same iff their indexes
        // are the same. This is ensured by the fact that our key types
        // implement Eq (as part of implementing Ord).
        if self.0 != Self::SENTINEL_VALUE && other.0 != Self::SENTINEL_VALUE {
            return self.0 == other.0;
        }

        // If any of the two indexes is the sentinel, we're required to perform
        // a lookup.
        CMP.with(|cmp| {
            let cmp = cmp.get().expect("cmp should be set");
            cmp(*self, *other) == Ordering::Equal
        })
    }
}

impl Eq for Index {}

impl Ord for Index {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // Ord should only be called if we're doing lookups within the table,
        // which should have set the thread local.
        CMP.with(|cmp| {
            let cmp = cmp.get().expect("cmp should be set");
            cmp(*self, *other)
        })
    }
}

impl PartialOrd for Index {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
