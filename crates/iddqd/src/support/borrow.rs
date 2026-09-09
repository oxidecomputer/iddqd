// Adapted from the Rust standard library, which is licensed under MIT OR
// Apache-2.0.
// Copyright (c) The Rust Project Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::{marker::PhantomData, ptr::NonNull};

/// Models a reborrow of some unique reference, when you know that the reborrow
/// and all its descendants (i.e., all pointers and references derived from it)
/// will not be used any more at some point, after which you want to use the
/// original unique reference again.
///
/// The borrow checker usually handles this stacking of borrows for you, but
/// some control flows that accomplish this stacking are too complicated for
/// the compiler to follow. A `DormantMutRef` allows you to check borrowing
/// yourself, while still expressing its stacked nature, and encapsulating
/// the raw pointer code needed to do this without undefined behavior.
///
/// # Where this is used
///
/// Only `IdOrdMap` uses this type, and only around a single item: it hashes
/// the item's key through a `&'a T` and then needs the `&'a mut T` back for
/// the `RefMut`. That stacking is invisible to the borrow checker because
/// `IdOrdItem::Key` requires only `Ord`, so the `Hash` bound names the full
/// lifetime `'a`.
///
/// Do not use this type to look up a caller-supplied key and then mutate the
/// map. If the caller's lookup type can name the lifetime of the mutable
/// borrow, the caller can keep a reference into the map across the awaken,
/// which is undefined behavior. The map-level lookup methods take owned keys
/// and shorten them with `upcast_key` instead; see the "Mutable lookups take
/// owned keys" section in the crate docs.
pub(crate) struct DormantMutRef<'a, T> {
    ptr: NonNull<T>,
    _marker: PhantomData<&'a mut T>,
}

// SAFETY: DormantMutRef<'a, T> stores exactly a reference to T. The "where"
// clause is that &mut T implements Sync.
unsafe impl<'a, T> Sync for DormantMutRef<'a, T> where &'a mut T: Sync {}

// SAFETY: DormantMutRef<'a, T> stores exactly a reference to T. The "where"
// clause is that &mut T implements Send.
unsafe impl<'a, T> Send for DormantMutRef<'a, T> where &'a mut T: Send {}

impl<'a, T> DormantMutRef<'a, T> {
    /// Capture a unique borrow, and immediately reborrow it. For the compiler,
    /// the lifetime of the new reference is the same as the lifetime of the
    /// original reference, but you promise to use it for a shorter period.
    pub(crate) fn new(t: &'a mut T) -> (&'a mut T, Self) {
        let ptr = NonNull::from(t);
        // SAFETY: we hold the borrow throughout 'a via `_marker`, and we expose
        // only this reference, so it is unique.
        let new_ref = unsafe { &mut *ptr.as_ptr() };
        (new_ref, Self { ptr, _marker: PhantomData })
    }

    /// Revert to the unique borrow initially captured.
    ///
    /// # Safety
    ///
    /// Every reference this `DormantMutRef` has handed out so far must be
    /// dead. That means the caller must no longer use:
    ///
    /// * the reference returned by `new`, and
    /// * any pointer or reference derived from it, including references that
    ///   caller-supplied code (such as a `Hash` or `Eq` impl) may have
    ///   retained.
    ///
    /// Each call retags from the raw pointer, which invalidates every earlier
    /// child. A use after that is undefined behavior.
    pub(crate) unsafe fn awaken(self) -> &'a mut T {
        // SAFETY: The caller promises that no earlier child of `ptr` will be
        // used again, so the reference we create here is the only live one.
        unsafe { &mut *self.ptr.as_ptr() }
    }
}
