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

// SAFETY:
//
// * When not armed, forwards to the wrapped allocator.
// * When armed, returns `AllocError` before touching the inner allocator, so no
//   pointer is fabricated.
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
