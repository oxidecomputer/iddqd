// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{support::hash_table::MapHash, TriHashMapEntry};
use std::ops::{Deref, DerefMut};

/// A mutable reference to a [`TriHashMap`] entry.
///
/// This is a wrapper around a `&mut T` that panics when dropped, if the
/// borrowed value's keys have changed since the wrapper was created.
///
/// # Change detection
///
/// It is illegal to change the keys of a borrowed `&mut T`. `RefMut` attempts
/// to enforce this invariant.
///
/// `RefMut` stores the `Hash` output of keys at creation time, and recomputes
/// these hashes when it is dropped or when [`Self::into_ref`] is called. If a
/// key changes, there's a small but non-negligible chance that its hash value
/// stays the same[^collision-chance]. In that case, as long as the new key is
/// not the same as another existing one, internal invariants are not violated
/// and the [`TriHashMap`] will continue to work correctly. (But don't do this!)
///
/// It is also possible to deliberately write pathological `Hash`
/// implementations that collide more often. (Don't do this either.)
///
/// Also, `RefMut`'s hash detection will not function if [`mem::forget`] is
/// called on it. If a key is changed and `mem::forget` is then called on the
/// `RefMut`, the `TriHashMap` will stop functioning correctly. This will not
/// introduce memory safety issues, however.
///
/// [`mem::forget`]: std::mem::forget
///
/// [^collision-chance]: The output of `Hash` is a [`u64`], so the probability
/// of an individual hash colliding by chance is 1/2⁶⁴. Due to the [birthday
/// problem], the probability of a collision by chance reaches 10⁻⁶ within
/// around 6 × 10⁶ elements.
///
/// [`TriHashMap`]: crate::TriHashMap
/// [birthday problem]: https://en.wikipedia.org/wiki/Birthday_problem#Probability_table
pub struct RefMut<'a, T: TriHashMapEntry> {
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: TriHashMapEntry> RefMut<'a, T> {
    pub(super) fn new(hashes: [MapHash; 3], borrowed: &'a mut T) -> Self {
        Self { inner: Some(RefMutInner { hashes, borrowed }) }
    }

    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<T: TriHashMapEntry> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<T: TriHashMapEntry> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<T: TriHashMapEntry> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

struct RefMutInner<'a, T: TriHashMapEntry> {
    hashes: [MapHash; 3],
    borrowed: &'a mut T,
}

impl<'a, T: TriHashMapEntry> RefMutInner<'a, T> {
    fn into_ref(self) -> &'a T {
        if !self.hashes[0].is_same_hash(self.borrowed.key1()) {
            panic!("key1 changed during RefMut borrow");
        }
        if !self.hashes[1].is_same_hash(self.borrowed.key2()) {
            panic!("key2 changed during RefMut borrow");
        }
        if !self.hashes[2].is_same_hash(self.borrowed.key3()) {
            panic!("key3 changed during RefMut borrow");
        }

        self.borrowed
    }
}

#[cfg(test)]
mod tests {
    use crate::{tri_hash_map::test_utils::TestEntry, TriHashMap};

    #[test]
    #[should_panic(expected = "key1 changed during RefMut borrow")]
    fn get_mut_panics_if_key1_changes() {
        let mut map = TriHashMap::<TestEntry>::new();
        map.insert_unique(TestEntry {
            key1: 128,
            key2: 'b',
            key3: "y".to_owned(),
            value: "x".to_owned(),
        })
        .unwrap();
        map.get1_mut(128).unwrap().key1 = 2;
    }

    #[test]
    #[should_panic(expected = "key2 changed during RefMut borrow")]
    fn get_mut_panics_if_key2_changes() {
        let mut map = TriHashMap::<TestEntry>::new();
        map.insert_unique(TestEntry {
            key1: 128,
            key2: 'b',
            key3: "y".to_owned(),
            value: "x".to_owned(),
        })
        .unwrap();
        map.get1_mut(128).unwrap().key2 = 'c';
    }

    #[test]
    #[should_panic(expected = "key3 changed during RefMut borrow")]
    fn get_mut_panics_if_key3_changes() {
        let mut map = TriHashMap::<TestEntry>::new();
        map.insert_unique(TestEntry {
            key1: 128,
            key2: 'b',
            key3: "y".to_owned(),
            value: "x".to_owned(),
        })
        .unwrap();
        map.get1_mut(128).unwrap().key3 = "z".to_owned();
    }
}
