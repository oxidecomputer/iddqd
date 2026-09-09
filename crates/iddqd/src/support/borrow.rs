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
    /// * the reference returned by `new`,
    /// * any reference returned by an earlier call to `reborrow` or
    ///   `reborrow_shared`, and
    /// * any pointer or reference derived from those.
    ///
    /// Each call retags from the raw pointer, which invalidates every earlier
    /// child. A use after that is undefined behavior.
    pub(crate) unsafe fn awaken(self) -> &'a mut T {
        // SAFETY: The caller promises that no earlier child of `ptr` will be
        // used again, so the reference we create here is the only live one.
        unsafe { &mut *self.ptr.as_ptr() }
    }

    /// Borrows a new mutable reference from the unique borrow initially
    /// captured.
    ///
    /// # Safety
    ///
    /// Same as [`Self::awaken`]: every reference this `DormantMutRef` has
    /// handed out so far must be dead. That means the caller must no longer
    /// use:
    ///
    /// * the reference returned by `new`,
    /// * any reference returned by an earlier call to `reborrow` or
    ///   `reborrow_shared`, and
    /// * any pointer or reference derived from those.
    pub(crate) unsafe fn reborrow(&mut self) -> &'a mut T {
        // SAFETY: The caller promises that no earlier child of `ptr` will be
        // used again, so the reference we create here is the only live one.
        unsafe { &mut *self.ptr.as_ptr() }
    }

    /// Borrows a new shared reference from the unique borrow initially
    /// captured.
    ///
    /// # Safety
    ///
    /// Every *mutable* reference this `DormantMutRef` has handed out so far
    /// must be dead. That means the caller must no longer use:
    ///
    /// * the reference returned by `new`,
    /// * any reference returned by an earlier call to `reborrow`, and
    /// * any pointer or reference derived from those.
    ///
    /// Shared references from earlier calls to `reborrow_shared` may still
    /// be in use. Shared references do not invalidate each other.
    pub(crate) unsafe fn reborrow_shared(&self) -> &'a T {
        // SAFETY: The caller promises that no earlier mutable child of `ptr`
        // will be used again. Earlier shared children may coexist with the
        // one we create here.
        unsafe { &*self.ptr.as_ptr() }
    }
}
