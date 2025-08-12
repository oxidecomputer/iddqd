use super::IdOrdItem;
use crate::support::map_hash::MapHash;
use core::{
    fmt,
    hash::{BuildHasher, Hash},
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
pub struct RefMut<'a, T: IdOrdItem> {
    inner: Option<RefMutInner<'a, T>>,
}

impl<'a, T: IdOrdItem> RefMut<'a, T>
where
    T::Key<'a>: Hash,
{
    pub(super) fn new(
        hash: MapHash<foldhash::fast::RandomState>,
        borrowed: &'a mut T,
    ) -> Self {
        let hash_value = hash.hash();
        let state = hash.into_state();
        let hash_one_fn = foldhash::fast::RandomState::hash_one::<T::Key<'a>>;

        // SAFETY: We cast this back to 'a, or to a lifetime shorter than that,
        // before using it.
        let hash_one_fn = hash_one_fn as *const ();

        let inner = RefMutInner { hash_value, state, hash_one_fn, borrowed };
        Self { inner: Some(inner) }
    }

    /// Converts this `RefMut` into a `&'a T`.
    pub fn into_ref(mut self) -> &'a T {
        let inner = self.inner.take().unwrap();
        inner.into_ref()
    }
}

impl<'a, T: IdOrdItem> RefMut<'a, T> {
    /// Borrows self into a shorter-lived `RefMut`.
    ///
    /// This `RefMut` will also check hash equality on drop.
    pub fn reborrow<'b>(&'b mut self) -> RefMut<'b, T>
    where
        T::Key<'b>: Hash,
        // Note: 'a: 'b is implicit because Self has the 'a parameter. (See
        // https://doc.rust-lang.org/nomicon/dropck.html.) We make
        // it explicit to be clear, though.
        'a: 'b,
    {
        let inner = self.inner.as_mut().unwrap();

        let borrowed = &mut *inner.borrowed;
        let inner = RefMutInner {
            hash_value: inner.hash_value,
            state: inner.state,
            hash_one_fn: inner.hash_one_fn,
            borrowed,
        };
        RefMut { inner: Some(inner) }
    }
}

impl<'a, T: IdOrdItem> Drop for RefMut<'a, T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.into_ref();
        }
    }
}

impl<'a, T: IdOrdItem> Deref for RefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().borrowed
    }
}

impl<'a, T: IdOrdItem> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().borrowed
    }
}

impl<'a, T: IdOrdItem + fmt::Debug> fmt::Debug for RefMut<'a, T> {
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
    hash_value: u64,
    // Store the hash_one function here so that type signatures aren't polluted with
    // T::Key<'a>: Hash everywhere. (A Drop impl, which is where the hash is
    // checked, cannot have stricter trait bounds than the type declaration.)
    //
    // We do pay the cost of dynamic dispatch here (i.e. not being able to
    // inline the hash function), but not having to say `T::Key<'a>: Hash`
    // everywhere allows `reborrow` to work against a non-'static T.
    state: foldhash::fast::RandomState,
    // In reality, this is a HashOneFn<'a, T>. But we store a raw pointer here
    // to avoid making 'a invariant.
    hash_one_fn: *const (),
    borrowed: &'a mut T,
}

impl<'a, T: IdOrdItem> RefMutInner<'a, T> {
    fn into_ref(self) -> &'a T {
        let key: T::Key<'_> = self.borrowed.key();
        // SAFETY: The key is borrowed, then dropped immediately. T is valid for
        // 'a so T::Key is valid for 'a.
        let key: T::Key<'a> =
            unsafe { std::mem::transmute::<T::Key<'_>, T::Key<'a>>(key) };

        let state = self.state;

        // SAFETY: We created hash_one_fn from one of:
        //
        // * a HashOneFn<'a, T>, or:
        // * a HashOneFn<'b, T> for some shorter lifetime 'b, through
        //   covariance.
        // * a HashOneFn<'b, T> for some shorter lifetime 'b, through the
        //   reborrow method.
        //
        // In all cases, the hash_one_fn is valid for the lifetime.
        let hash_one_fn = unsafe {
            core::mem::transmute::<*const (), HashOneFn<'a, T>>(
                self.hash_one_fn,
            )
        };

        let hash = (hash_one_fn)(&state, key);

        if self.hash_value != hash {
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

type HashOneFn<'a, T> =
    fn(&foldhash::fast::RandomState, <T as IdOrdItem>::Key<'a>) -> u64;
