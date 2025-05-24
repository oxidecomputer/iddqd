use super::IdOrdItem;
use crate::support::map_hash::MapHash;
use core::{
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
};

/// A mutable reference to an [`IdOrdMap`] entry.
///
/// This is a wrapper around a `&mut T` that panics when dropped, if the
/// borrowed value's key has changed since the wrapper was created.
///
/// # Change detection
///
/// It is illegal to change the keys of a borrowed `&mut T`. `RefMut` attempts
/// to enforce this invariant, and as part of that, it requires that the key
/// type implement [`Hash`].
///
/// `RefMut` stores the `Hash` output of keys at creation time, and recomputes
/// these hashes when it is dropped or when [`Self::into_ref`] is called. If a
/// key changes, there's a small but non-negligible chance that its hash value
/// stays the same[^collision-chance]. In that case, the map will no longer
/// function correctly and might panic on access. This will not introduce memory
/// safety issues, however.
///
/// It is also possible to deliberately write pathological `Hash`
/// implementations that collide more often. (Don't do this.)
///
/// Also, `RefMut`'s hash detection will not function if [`mem::forget`] is
/// called on it. If a key is changed and `mem::forget` is then called on the
/// `RefMut`, the [`IdOrdMap`] will no longer function correctly and might panic
/// on access. This will not introduce memory safety issues, however.
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
/// [`IdOrdMap`]: crate::IdOrdMap
/// [birthday problem]: https://en.wikipedia.org/wiki/Birthday_problem#Probability_table
pub struct RefMut<'a, T: IdOrdItem>
where
    for<'k> T::Key<'k>: Hash,
{
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: IdOrdItem> RefMut<'a, T>
where
    for<'k> T::Key<'k>: Hash,
{
    pub(super) fn new(
        hash: MapHash<foldhash::fast::RandomState>,
        borrowed: &'a mut T,
    ) -> Self {
        let inner = RefMutInner { hash, borrowed };
        Self { inner: Some(inner) }
    }

    /// Borrows self into a shorter-lived `RefMut`.
    ///
    /// This `RefMut` will also check hash equality on drop.
    pub fn reborrow(&mut self) -> RefMut<'_, T> {
        let inner = self.inner.as_mut().unwrap();
        let borrowed = &mut *inner.borrowed;
        RefMut::new(inner.hash.clone(), borrowed)
    }

    /// Converts this `RefMut` into a `&'a T`.
    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<T: IdOrdItem> Drop for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<T: IdOrdItem> Deref for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<T: IdOrdItem> DerefMut for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

impl<T: IdOrdItem + fmt::Debug> fmt::Debug for RefMut<'_, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            Some(ref inner) => inner.fmt(f),
            None => {
                f.debug_struct("RefMut").field("borrowed", &"missing").finish()
            }
        }
    }
}

struct RefMutInner<'a, T: IdOrdItem> {
    hash: MapHash<foldhash::fast::RandomState>,
    borrowed: &'a mut T,
}

impl<'a, T: IdOrdItem> RefMutInner<'a, T>
where
    for<'k> T::Key<'k>: Hash,
{
    fn into_ref(self) -> &'a T {
        if !self.hash.is_same_hash(self.borrowed.key()) {
            panic!("key changed during RefMut borrow");
        }

        self.borrowed
    }
}

impl<T: IdOrdItem + fmt::Debug> fmt::Debug for RefMutInner<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.borrowed.fmt(f)
    }
}
