//! A "table" of b-tree-based indexes.
//!
//! Similar to [`super::hash_table::MapHashTable`], b-tree based tables store
//! integers (that are indexes corresponding to items), but use an external
//! comparator.

use super::map_hash::{HashState, MapHash};
use crate::internal::{TableValidationError, ValidateCompact};
use alloc::{
    collections::{BTreeSet, btree_set},
    vec::Vec,
};
use core::{
    borrow::Borrow,
    cmp::Ordering,
    hash::{BuildHasher, Hash},
};
use equivalent::Comparable;

/// A B-tree-based table with an external comparator.
#[derive(Clone, Debug, Default)]
pub(crate) struct MapBTreeTable {
    items: BTreeSet<Index>,
    hash_state: HashState,
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
        K: Ord,
        Q: ?Sized + Comparable<K>,
        F: Fn(usize) -> K,
    {
        let f = find_cmp(key, lookup);
        let cmp_wrapper =
            CmpWrapper { index: Index::SENTINEL, cmp_fn: Some(&f) };

        let ret = match self.items.get(&cmp_wrapper as &dyn CmpKey<_>) {
            Some(Index(v)) if *v == Index::SENTINEL_VALUE => {
                panic!("internal map shouldn't store sentinel value")
            }
            Some(Index(v)) => Some(*v),
            None => {
                // The key is not in the table.
                None
            }
        };

        ret
    }

    pub(crate) fn insert<K, Q, F>(&mut self, index: usize, key: &Q, lookup: F)
    where
        K: Ord,
        Q: ?Sized + Comparable<K>,
        F: Fn(usize) -> K,
    {
        let f = insert_cmp(index, key, lookup);
        let index = Index::new(index);
        let cmp_wrapper = CmpWrapper { index, cmp_fn: Some(&f) };

        self.items
            .get_or_insert_with(&cmp_wrapper as &dyn CmpKey<_>, |_| index);
    }

    pub(crate) fn remove<K, F>(&mut self, index: usize, key: K, lookup: F)
    where
        F: Fn(usize) -> K,
        K: Ord,
    {
        let f = insert_cmp(index, &key, lookup);
        let find_cmp =
            CmpWrapper { index: Index::new(index), cmp_fn: Some(&f) };

        self.items.remove(&find_cmp as &dyn CmpKey<_>);
    }

    pub(crate) fn iter(&self) -> Iter {
        Iter::new(self.items.iter())
    }

    pub(crate) fn into_iter(self) -> IntoIter {
        IntoIter::new(self.items.into_iter())
    }

    pub(crate) fn compute_hash<K: Hash>(&self, key: K) -> MapHash {
        MapHash { state: self.hash_state, hash: self.hash_state.hash_one(key) }
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
    Q: ?Sized + Comparable<K>,
    F: 'a + Fn(usize) -> K,
    K: Ord,
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
            (Index::SENTINEL_VALUE, v) => key.compare(&lookup(v)),
            (v, Index::SENTINEL_VALUE) => key.compare(&lookup(v)).reverse(),
            (a, b) => lookup(a).cmp(&lookup(b)),
        }
    }
}

fn insert_cmp<'a, K, Q, F>(
    index: usize,
    key: &'a Q,
    lookup: F,
) -> impl Fn(Index, Index) -> Ordering + 'a
where
    Q: ?Sized + Comparable<K>,
    F: 'a + Fn(usize) -> K,
    K: Ord,
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
            (a, b) if a == index => key.compare(&lookup(b)),
            (a, b) if b == index => key.compare(&lookup(a)).reverse(),
            (a, b) => lookup(a).cmp(&lookup(b)),
        }
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

        panic!("we should never call PartialEq on indexes");
    }
}

impl Eq for Index {}

impl Ord for Index {
    #[inline]
    fn cmp(&self, _other: &Self) -> Ordering {
        panic!("we should never call Ord on indexes");
    }
}

impl PartialOrd for Index {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct CmpWrapper<'a, F> {
    index: Index,
    cmp_fn: Option<&'a F>,
}

impl<F> Clone for CmpWrapper<'_, F> {
    fn clone(&self) -> Self {
        Self { index: self.index, cmp_fn: self.cmp_fn }
    }
}

impl<F> Copy for CmpWrapper<'_, F> {}

trait CmpKey<F> {
    fn key(&self) -> CmpWrapper<'_, F>;
}

impl<F> CmpKey<F> for Index {
    fn key(&self) -> CmpWrapper<'_, F> {
        CmpWrapper { index: *self, cmp_fn: None }
    }
}

impl<'a, F> CmpKey<F> for CmpWrapper<'a, F> {
    fn key(&self) -> CmpWrapper<'_, F> {
        *self
    }
}

impl<'a, F> Borrow<dyn CmpKey<F> + 'a> for Index {
    fn borrow(&self) -> &(dyn CmpKey<F> + 'a) {
        self
    }
}

impl<'a, F: Fn(Index, Index) -> Ordering> PartialEq for (dyn CmpKey<F> + 'a) {
    fn eq(&self, other: &Self) -> bool {
        let key = self.key();
        let other_key = other.key();
        // At least one of the cmp fns must be set.
        let cmp = key
            .cmp_fn
            .or_else(|| other_key.cmp_fn)
            .expect("at least one key must be set");
        cmp(key.index, other_key.index) == Ordering::Equal
    }
}

impl<'a, F: Fn(Index, Index) -> Ordering> Eq for (dyn CmpKey<F> + 'a) {}

impl<'a, F: Fn(Index, Index) -> Ordering> Ord for (dyn CmpKey<F> + 'a) {
    fn cmp(&self, other: &Self) -> Ordering {
        let key = self.key();
        let other_key = other.key();
        // At least one of the cmp fns must be set.
        let cmp = key
            .cmp_fn
            .or_else(|| other_key.cmp_fn)
            .expect("at least one key must be set");
        cmp(key.index, other_key.index)
    }
}

impl<'a, F: Fn(Index, Index) -> Ordering> PartialOrd for (dyn CmpKey<F> + 'a) {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
