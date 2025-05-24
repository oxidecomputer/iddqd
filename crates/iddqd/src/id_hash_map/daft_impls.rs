//! `Diffable` implementation.

use super::{IdHashItem, IdHashMap};
use crate::{DefaultHashBuilder, support::daft_utils::IdLeaf};
use core::hash::{BuildHasher, Hash};
use daft::Diffable;
use derive_where::derive_where;
use equivalent::Equivalent;

impl<T: IdHashItem, S: Clone + BuildHasher> Diffable for IdHashMap<T, S> {
    type Diff<'a>
        = Diff<'a, T, S>
    where
        T: 'a,
        S: 'a;

    fn diff<'daft>(&'daft self, other: &'daft Self) -> Self::Diff<'daft> {
        let mut diff = Diff::with_hasher(self.hasher().clone());
        for item in self {
            if let Some(other_item) = other.get(&item.key()) {
                diff.common.insert_overwrite(IdLeaf::new(item, other_item));
            } else {
                diff.removed.insert_overwrite(item);
            }
        }
        for item in other {
            if !self.contains_key(&item.key()) {
                diff.added.insert_overwrite(item);
            }
        }
        diff
    }
}

/// A diff of two [`IdHashMap`]s.
#[derive_where(Default; S: Default)]
pub struct Diff<'daft, T: ?Sized + IdHashItem, S = DefaultHashBuilder> {
    /// Entries common to both maps.
    ///
    /// Items are stored as [`IdLeaf`]s to references.
    pub common: IdHashMap<IdLeaf<&'daft T>, S>,

    /// Added entries.
    pub added: IdHashMap<&'daft T, S>,

    /// Removed entries.
    pub removed: IdHashMap<&'daft T, S>,
}

#[cfg(feature = "default-hasher")]
impl<'daft, T: ?Sized + IdHashItem> Diff<'daft, T> {
    /// Creates a new, empty `IdHashMapDiff`
    pub fn new() -> Self {
        Self {
            common: IdHashMap::new(),
            added: IdHashMap::new(),
            removed: IdHashMap::new(),
        }
    }
}

impl<'daft, T: ?Sized + IdHashItem, S: Clone + BuildHasher> Diff<'daft, T, S> {
    /// Creates a new `IdHashMapDiff` with the given hasher.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            common: IdHashMap::with_hasher(hasher.clone()),
            added: IdHashMap::with_hasher(hasher.clone()),
            removed: IdHashMap::with_hasher(hasher),
        }
    }
}

impl<'daft, T: ?Sized + IdHashItem + Eq, S: Clone + BuildHasher>
    Diff<'daft, T, S>
{
    /// Returns an iterator over unchanged keys and values.
    pub fn unchanged(&self) -> impl Iterator<Item = &'daft T> + '_ {
        self.common
            .iter()
            .filter_map(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns true if the item corresponding to the key is unchanged.
    pub fn is_unchanged<'a, Q>(&'a self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.common.get(key).is_some_and(|leaf| leaf.is_unchanged())
    }

    /// Returns the value associated with the key if it is unchanged,
    /// otherwise `None`.
    pub fn get_unchanged<'a, Q>(&'a self, key: &Q) -> Option<&'daft T>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.common
            .get(key)
            .and_then(|leaf| leaf.is_unchanged().then_some(*leaf.before()))
    }

    /// Returns an iterator over modified keys and values.
    pub fn modified(&self) -> impl Iterator<Item = IdLeaf<&'daft T>> + '_ {
        self.common
            .iter()
            .filter_map(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns true if the value corresponding to the key is
    /// modified.
    pub fn is_modified<'a, Q>(&'a self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.common.get(key).is_some_and(|leaf| leaf.is_modified())
    }

    /// Returns the [`IdLeaf`] associated with the key if it is modified,
    /// otherwise `None`.
    pub fn get_modified<'a, Q>(&'a self, key: &Q) -> Option<IdLeaf<&'daft T>>
    where
        Q: ?Sized + Hash + Equivalent<T::Key<'a>>,
    {
        self.common
            .get(key)
            .and_then(|leaf| leaf.is_modified().then_some(*leaf))
    }

    /// Returns an iterator over modified keys and values, performing
    /// a diff on the values.
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

impl<T: IdHashItem> IdHashItem for IdLeaf<T> {
    type Key<'a>
        = T::Key<'a>
    where
        T: 'a;

    fn key(&self) -> Self::Key<'_> {
        let before_key = self.before().key();
        if before_key != self.after().key() {
            panic!("key is different between before and after");
        }
        self.before().key()
    }

    #[inline]
    fn upcast_key<'short, 'long: 'short>(
        long: Self::Key<'long>,
    ) -> Self::Key<'short> {
        T::upcast_key(long)
    }
}
