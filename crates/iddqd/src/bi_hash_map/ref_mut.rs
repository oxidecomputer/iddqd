use crate::{BiHashItem, support::map_hash::MapHash};
use core::{
    fmt,
    ops::{Deref, DerefMut},
};

/// A mutable reference to a [`BiHashMap`] item.
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
/// and the [`BiHashMap`] will continue to work correctly. (But don't do this!)
///
/// It is also possible to deliberately write pathological `Hash`
/// implementations that collide more often. (Don't do this either.)
///
/// Also, `RefMut`'s hash detection will not function if [`mem::forget`] is
/// called on it. If a key is changed and `mem::forget` is then called on the
/// `RefMut`, the `BiHashMap` will stop functioning correctly. This will not
/// introduce memory safety issues, however.
///
/// The issues here are similar to using interior mutability (e.g. `RefCell` or
/// `Mutex`) to mutate keys in a regular `HashMap`.
///
/// [`mem::forget`]: std::mem::forget
///
/// [^collision-chance]: The output of `Hash` is a [`u64`], so the probability
/// of an individual hash colliding by chance is 1/2⁶⁴. Due to the [birthday
/// problem], the probability of a collision by chance reaches 10⁻⁶ within
/// around 6 × 10⁶ elements.
///
/// [`BiHashMap`]: crate::BiHashMap
/// [birthday problem]: https://en.wikipedia.org/wiki/Birthday_problem#Probability_table
pub struct RefMut<'a, T: BiHashItem> {
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: BiHashItem> RefMut<'a, T> {
    pub(super) fn new(hashes: [MapHash; 2], borrowed: &'a mut T) -> Self {
        Self { inner: Some(RefMutInner { hashes, borrowed }) }
    }

    /// Borrows self into a shorter-lived `RefMut`.
    ///
    /// This `RefMut` will also check hash equality on drop.
    pub fn reborrow(&mut self) -> RefMut<'_, T> {
        let inner = self.inner.as_mut().unwrap();
        let borrowed = &mut *inner.borrowed;
        RefMut::new(inner.hashes.clone(), borrowed)
    }

    /// Converts this `RefMut` into a `&'a T`.
    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<T: BiHashItem> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<T: BiHashItem> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<T: BiHashItem> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

impl<T: BiHashItem + fmt::Debug> fmt::Debug for RefMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            Some(ref inner) => inner.fmt(f),
            None => {
                f.debug_struct("RefMut").field("borrowed", &"missing").finish()
            }
        }
    }
}

struct RefMutInner<'a, T: BiHashItem> {
    hashes: [MapHash; 2],
    borrowed: &'a mut T,
}

impl<'a, T: BiHashItem> RefMutInner<'a, T> {
    fn into_ref(self) -> &'a T {
        if !self.hashes[0].is_same_hash(self.borrowed.key1()) {
            panic!("key1 changed during RefMut borrow");
        }
        if !self.hashes[1].is_same_hash(self.borrowed.key2()) {
            panic!("key2 changed during RefMut borrow");
        }

        self.borrowed
    }
}

impl<T: BiHashItem + fmt::Debug> fmt::Debug for RefMutInner<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrowed.fmt(f)
    }
}
