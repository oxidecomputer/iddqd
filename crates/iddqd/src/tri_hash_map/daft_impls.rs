//! `Diffable` implementation.

use super::{TriHashItem, TriHashMap};
use crate::support::daft_utils::IdLeaf;
use core::{borrow::Borrow, hash::Hash};
use daft::Diffable;

impl<T: TriHashItem> Diffable for TriHashMap<T> {
    type Diff<'a>
        = Diff<'a, T>
    where
        T: 'a;

    fn diff<'daft>(&'daft self, other: &'daft Self) -> Self::Diff<'daft> {
        let mut diff = Diff::new();
        for item in self {
            if let Some(other_item) =
                other.get_unique(&item.key1(), &item.key2(), &item.key3())
            {
                diff.common.insert_overwrite(IdLeaf::new(item, other_item));
            } else {
                diff.removed.insert_overwrite(item);
            }
        }
        for item in other {
            if !self.contains_key_unique(
                &item.key1(),
                &item.key2(),
                &item.key3(),
            ) {
                diff.added.insert_overwrite(item);
            }
        }
        diff
    }
}

/// A diff of two [`TriHashMap`]s.
pub struct Diff<'daft, T: ?Sized + TriHashItem> {
    /// Entries common to both maps.
    ///
    /// Items are stored as [`IdLeaf`]s to references.
    pub common: TriHashMap<IdLeaf<&'daft T>>,

    /// Added entries.
    pub added: TriHashMap<&'daft T>,

    /// Removed entries.
    pub removed: TriHashMap<&'daft T>,
}

impl<'daft, T: ?Sized + TriHashItem> Diff<'daft, T> {
    /// Creates a new `TriHashMapDiff`.
    pub fn new() -> Self {
        Self {
            common: TriHashMap::new(),
            added: TriHashMap::new(),
            removed: TriHashMap::new(),
        }
    }
}

impl<'daft, T: ?Sized + TriHashItem + Eq> Diff<'daft, T> {
    /// Returns an iterator over unchanged keys and values.
    pub fn unchanged(&self) -> impl Iterator<Item = &'daft T> + '_ {
        self.common
            .iter()
            .filter_map(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns true if the item corresponding to `key1` is unchanged.
    pub fn is_unchanged1<'a, Q>(&'a self, key1: &Q) -> bool
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get1(key1).is_some_and(|leaf| leaf.is_unchanged())
    }

    /// Returns true if the item corresponding to `key2` is unchanged.
    pub fn is_unchanged2<'a, Q>(&'a self, key2: &Q) -> bool
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get2(key2).is_some_and(|leaf| leaf.is_unchanged())
    }

    /// Returns true if the item corresponding to `key3` is unchanged.
    pub fn is_unchanged3<'a, Q>(&'a self, key3: &Q) -> bool
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get3(key3).is_some_and(|leaf| leaf.is_unchanged())
    }

    /// Returns the value associated with `key1` if it is unchanged,
    /// otherwise `None`.
    pub fn get_unchanged1<'a, Q>(&'a self, key: &Q) -> Option<&'daft T>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get1(key)
            .and_then(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns the value associated with `key2` if it is unchanged,
    /// otherwise `None`.
    pub fn get_unchanged2<'a, Q>(&'a self, key: &Q) -> Option<&'daft T>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get2(key)
            .and_then(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns the value associated with `key3` if it is unchanged,
    /// otherwise `None`.
    pub fn get_unchanged3<'a, Q>(&'a self, key: &Q) -> Option<&'daft T>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get3(key)
            .and_then(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns an iterator over modified keys and values.
    pub fn modified(&self) -> impl Iterator<Item = IdLeaf<&'daft T>> + '_ {
        self.common
            .iter()
            .filter_map(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns true if the value corresponding to `key1` is modified.
    pub fn is_modified1<'a, Q>(&'a self, key1: &Q) -> bool
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get1(key1).is_some_and(|leaf| leaf.is_modified())
    }

    /// Returns true if the value corresponding to `key2` is modified.
    pub fn is_modified2<'a, Q>(&'a self, key2: &Q) -> bool
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get2(key2).is_some_and(|leaf| leaf.is_modified())
    }

    /// Returns true if the value corresponding to `key3` is modified.
    pub fn is_modified3<'a, Q>(&'a self, key3: &Q) -> bool
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common.get3(key3).is_some_and(|leaf| leaf.is_modified())
    }

    /// Returns the [`IdLeaf`] associated with `key1` if it is modified,
    /// otherwise `None`.
    pub fn get_modified1<'a, Q>(&'a self, key: &Q) -> Option<IdLeaf<&'daft T>>
    where
        T::K1<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get1(key)
            .and_then(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns the [`IdLeaf`] associated with `key2` if it is modified,
    /// otherwise `None`.
    pub fn get_modified2<'a, Q>(&'a self, key: &Q) -> Option<IdLeaf<&'daft T>>
    where
        T::K2<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get2(key)
            .and_then(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns the [`IdLeaf`] associated with `key3` if it is modified,
    /// otherwise `None`.
    pub fn get_modified3<'a, Q>(&'a self, key: &Q) -> Option<IdLeaf<&'daft T>>
    where
        T::K3<'a>: Borrow<Q>,
        T: 'a,
        Q: Hash + Eq + ?Sized,
    {
        self.common
            .get3(key)
            .and_then(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns an iterator over modified keys and values, performing a diff on
    /// the values.
    ///
    /// This is useful when `T::Diff` is a complex type, not just a
    /// [`daft::Leaf`].
    pub fn modified_diff(&self) -> impl Iterator<Item = T::Diff<'daft>> + '_
    where
        T: Diffable,
    {
        self.modified().map(|leaf| leaf.diff_pair())
    }
}

// Note: not deriving Default here because we don't want to require
// T to be Default.
impl<'daft, T: TriHashItem> Default for Diff<'daft, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TriHashItem> TriHashItem for IdLeaf<T> {
    type K1<'a>
        = T::K1<'a>
    where
        T: 'a;
    type K2<'a>
        = T::K2<'a>
    where
        T: 'a;
    type K3<'a>
        = T::K3<'a>
    where
        T: 'a;

    fn key1(&self) -> Self::K1<'_> {
        let before_key = self.before().key1();
        if before_key != self.after().key1() {
            panic!("key1 is different between before and after");
        }
        self.before().key1()
    }

    fn key2(&self) -> Self::K2<'_> {
        let before_key = self.before().key2();
        if before_key != self.after().key2() {
            panic!("key2 is different between before and after");
        }
        self.before().key2()
    }

    fn key3(&self) -> Self::K3<'_> {
        let before_key = self.before().key3();
        if before_key != self.after().key3() {
            panic!("key3 is different between before and after");
        }
        self.before().key3()
    }

    #[inline]
    fn upcast_key1<'short, 'long: 'short>(
        long: Self::K1<'long>,
    ) -> Self::K1<'short> {
        T::upcast_key1(long)
    }

    #[inline]
    fn upcast_key2<'short, 'long: 'short>(
        long: Self::K2<'long>,
    ) -> Self::K2<'short> {
        T::upcast_key2(long)
    }

    #[inline]
    fn upcast_key3<'short, 'long: 'short>(
        long: Self::K3<'long>,
    ) -> Self::K3<'short> {
        T::upcast_key3(long)
    }
}
