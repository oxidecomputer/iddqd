//! An allocator that injects allocation failures, for testing that fallible
//! reservation paths stay atomic when the system allocator reports OOM.

use allocator_api2::alloc::{AllocError, Allocator, Layout};
use core::{cell::Cell, ptr::NonNull};

thread_local! {
    static ALLOC_SHOULD_FAIL: Cell<bool> = const { Cell::new(false) };
}

pub fn with_failing_alloc<R>(f: impl FnOnce() -> R) -> R {
    struct ResetGuard;
    impl Drop for ResetGuard {
        fn drop(&mut self) {
            ALLOC_SHOULD_FAIL.with(|c| c.set(false));
        }
    }

    ALLOC_SHOULD_FAIL.with(|c| c.set(true));
    let _guard = ResetGuard;
    f()
}

/// An allocator that returns [`AllocError`] for every allocation made inside
/// [`with_failing_alloc`], and otherwise forwards to the wrapped allocator.
#[derive(Clone, Copy, Debug)]
pub struct FailingAlloc<A>(pub A);

// SAFETY: The `Allocator` contract requires that blocks returned from this
// allocator stay valid until deallocated, and that a clone or copy of the
// allocator behaves identically (so a block from one can be freed through
// another).
//
// Every block `FailingAlloc` hands out comes from the wrapped allocator `A`:
// `allocate` returns either `Err` or exactly `self.0.allocate`'s result, and
// `deallocate` forwards to `self.0`. `#[derive(Clone, Copy)]` clones the inner
// `A`, so block validity and clone-stability inherit entirely from `A`.
// ALLOC_SHOULD_FAIL only chooses whether to fail before delegating -- it never
// touches an already-allocated block.
unsafe impl<A: Allocator> Allocator for FailingAlloc<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if ALLOC_SHOULD_FAIL.with(|c| c.get()) {
            return Err(AllocError);
        }
        self.0.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: inherits from the wrapped allocator's contract.
        unsafe { self.0.deallocate(ptr, layout) }
    }
}
