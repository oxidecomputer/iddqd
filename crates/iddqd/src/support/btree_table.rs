// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A "table" of b-tree-based indexes.
//!
//! Similar to [`super::hash_table::MapHashTable`], b-tree based tables store
//! integers (that are indexes corresponding to entries), but use an external
//! comparator.

use std::{
    borrow::Borrow,
    cell::Cell,
    cmp::Ordering,
    collections::{btree_set, BTreeSet},
    marker::PhantomData,
};

thread_local! {
    static CMP: Cell<Option<&'static dyn Fn(Index, Index) -> Ordering>>
        = const { Cell::new(None) };
}

/// A B-tree-based table with an external comparator.
#[derive(Clone, Debug, Default)]
pub(crate) struct MapBTreeTable {
    entries: BTreeSet<Index>,
}

impl MapBTreeTable {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn validate(
        &self,
        expected_len: usize,
        compactness: crate::test_utils::ValidateCompact,
    ) -> anyhow::Result<()> {
        use crate::test_utils::ValidateCompact;
        use anyhow::{bail, ensure};

        ensure!(
            self.len() == expected_len,
            "expected length {expected_len}, was {}",
            self.len()
        );

        match compactness {
            ValidateCompact::Compact => {
                // All entries between 0 (inclusive) and self.len() (exclusive)
                // are present, and there are no duplicates. Also, the sentinel
                // value should not be stored.
                let mut indexes: Vec<_> = Vec::with_capacity(expected_len);
                for index in &self.entries {
                    match index.0 {
                        Index::SENTINEL_VALUE => {
                            bail!("index should not be used in path");
                        }
                        v => {
                            indexes.push(v);
                        }
                    }
                }
                indexes.sort_unstable();
                for (i, index) in indexes.iter().enumerate() {
                    ensure!(
                        *index == i,
                        "value at index {i} should be {i}, was {index}",
                    );
                }
            }
            ValidateCompact::NonCompact => {
                // There should be no duplicates, and the sentinel value
                // should not be stored.
                let values: Vec<_> = self.entries.iter().copied().collect();
                let value_set: BTreeSet<usize> =
                    values.iter().map(|ix| ix.0).collect();
                ensure!(
                    value_set.len() == values.len(),
                    "expected no duplicates, but found {} duplicates \
                     (values: {:?})",
                    values.len() - value_set.len(),
                    values,
                );
                ensure!(
                    !value_set.contains(&Index::SENTINEL_VALUE),
                    "expected sentinel value to be absent from the set, \
                     but found it"
                );
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

        let ret = match self.entries.get(&Index::SENTINEL) {
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

        self.entries.insert(Index::new(index));

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

        self.entries.remove(&Index::new(index));

        // drop(guard) isn't necessary, but we make it explicit
        drop(guard);
    }

    pub(crate) fn iter(&self) -> Iter {
        Iter::new(self.entries.iter())
    }

    pub(crate) fn into_iter(self) -> IntoIter {
        IntoIter::new(self.entries.into_iter())
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

fn find_cmp<K, Q, F>(
    key: &Q,
    lookup: F,
) -> impl Fn(Index, Index) -> Ordering + use<'_, K, Q, F>
where
    F: Fn(usize) -> K,
    K: Ord + Borrow<Q>,
    Q: ?Sized + Ord,
{
    move |a: Index, b: Index| match (a.0, b.0) {
        (Index::SENTINEL_VALUE, Index::SENTINEL_VALUE) => Ordering::Equal,
        (Index::SENTINEL_VALUE, v) => key.borrow().cmp(lookup(v).borrow()),
        (v, Index::SENTINEL_VALUE) => lookup(v).borrow().cmp(key.borrow()),
        (a, b) => lookup(a).borrow().cmp(lookup(b).borrow()),
    }
}

fn insert_cmp<K, Q, F>(
    index: usize,
    key: &Q,
    lookup: F,
) -> impl Fn(Index, Index) -> Ordering + use<'_, K, Q, F>
where
    F: Fn(usize) -> K,
    K: Ord + Borrow<Q>,
    Q: ?Sized + Ord,
{
    move |a: Index, b: Index| match (a.0, b.0) {
        // The sentinel value should not be invoked at all, because it's not
        // passed in during insert and not stored in the table.
        (Index::SENTINEL_VALUE, _) | (_, Index::SENTINEL_VALUE) => {
            panic!("sentinel value should not be invoked in insert path")
        }
        (a, b) => {
            if a == b {
                return Ordering::Equal;
            }
            match (a, b) {
                (a, b) if a == index => key.borrow().cmp(lookup(b).borrow()),
                (a, b) if b == index => lookup(a).borrow().cmp(key.borrow()),
                (a, b) => lookup(a).borrow().cmp(lookup(b).borrow()),
            }
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
