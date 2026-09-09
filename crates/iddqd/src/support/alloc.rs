// Adapted from the hashbrown crate, which is licensed under MIT OR Apache-2.0.
// Copyright (c) 2016-2025 Amanieu d'Antras and others
// SPDX-License-Identifier: MIT OR Apache-2.0

pub use self::inner::Global;
pub(crate) use self::inner::{AllocWrapper, Allocator, global_alloc};

// TODO: support nightly.

// Note that `AllocWrapper` forwards every method of the allocator trait, not
// just `allocate` and `deallocate`. (This is a change from hashbrown.)
//
// `ItemSet` stores items in an allocator-api2 `Vec`, which grows and shrinks
// through `grow` and `shrink`. If those fell through to the trait's default
// implementations, a `Vec` resize would call the user's `deallocate` on the old
// block partway through `grow`, rather than the user's own `grow`. A user
// allocator whose `deallocate` unwinds after freeing would then leave the `Vec`
// holding a freed pointer, and dropping the map would free it again.

// Basic non-nightly case.
#[cfg(feature = "allocator-api2")]
mod inner {
    use allocator_api2::alloc::AllocError;
    pub use allocator_api2::alloc::{Allocator, Global, Layout};
    use core::ptr::NonNull;

    #[inline]
    pub(crate) const fn global_alloc() -> Global {
        Global
    }

    #[derive(Clone, Copy, Default)]
    pub(crate) struct AllocWrapper<T>(pub(crate) T);

    // SAFETY: Every method forwards to the wrapped allocator with the same
    // arguments, so each one inherits the wrapped allocator's guarantees and
    // preconditions unchanged.
    unsafe impl<T: Allocator> allocator_api2::alloc::Allocator for AllocWrapper<T> {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            Allocator::allocate(&self.0, layout)
        }

        #[inline]
        fn allocate_zeroed(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            Allocator::allocate_zeroed(&self.0, layout)
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::deallocate(&self.0, ptr, layout) }
        }

        #[inline]
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::grow(&self.0, ptr, old_layout, new_layout) }
        }

        #[inline]
        unsafe fn grow_zeroed(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                Allocator::grow_zeroed(&self.0, ptr, old_layout, new_layout)
            }
        }

        #[inline]
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::shrink(&self.0, ptr, old_layout, new_layout) }
        }
    }
}

// No-defaults case.
#[cfg(not(feature = "allocator-api2"))]
mod inner {
    use crate::alloc::alloc::Layout;
    use allocator_api2::alloc::AllocError;
    use core::ptr::NonNull;

    #[inline]
    pub(crate) const fn global_alloc() -> Global {
        Global::new()
    }

    /// A stand-in for `allocator_api2::alloc::Allocator` when the
    /// `allocator-api2` feature is off.
    ///
    /// This trait lives in a private module and is only re-exported at
    /// `pub(crate)`, so nothing outside this crate can name or implement it.
    /// The only implementation is [`Global`], which forwards to
    /// `allocator_api2::alloc::Global`.
    ///
    /// The method set matches `allocator_api2::alloc::Allocator` so that
    /// `AllocWrapper` can forward every method rather than fall through to
    /// the trait's default implementations (see the module comment).
    ///
    /// # Safety
    ///
    /// Implementations must uphold the contract of
    /// `allocator_api2::alloc::Allocator`. `AllocWrapper` forwards to this
    /// trait when it implements that one, so an implementation here that
    /// broke the contract would break hashbrown and `Vec`.
    pub unsafe trait Allocator {
        fn allocate(&self, layout: Layout)
        -> Result<NonNull<[u8]>, AllocError>;
        fn allocate_zeroed(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError>;
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError>;
        unsafe fn grow_zeroed(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError>;
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError>;
    }

    #[derive(Copy, Clone, Default)]
    #[doc(hidden)]
    pub struct Global(allocator_api2::alloc::Global);

    impl Global {
        #[inline]
        pub const fn new() -> Self {
            Global(allocator_api2::alloc::Global)
        }
    }

    // SAFETY: Every method forwards to `allocator_api2::alloc::Global` with
    // the same arguments.
    unsafe impl Allocator for Global {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            allocator_api2::alloc::Allocator::allocate(&self.0, layout)
        }

        #[inline]
        fn allocate_zeroed(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            allocator_api2::alloc::Allocator::allocate_zeroed(&self.0, layout)
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                allocator_api2::alloc::Allocator::deallocate(
                    &self.0, ptr, layout,
                );
            }
        }

        #[inline]
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                allocator_api2::alloc::Allocator::grow(
                    &self.0, ptr, old_layout, new_layout,
                )
            }
        }

        #[inline]
        unsafe fn grow_zeroed(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                allocator_api2::alloc::Allocator::grow_zeroed(
                    &self.0, ptr, old_layout, new_layout,
                )
            }
        }

        #[inline]
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                allocator_api2::alloc::Allocator::shrink(
                    &self.0, ptr, old_layout, new_layout,
                )
            }
        }
    }

    #[derive(Clone, Copy, Default)]
    pub(crate) struct AllocWrapper<T>(pub(crate) T);

    // SAFETY: Every method forwards to the wrapped allocator with the same
    // arguments, so each one inherits the wrapped allocator's guarantees and
    // preconditions unchanged.
    unsafe impl<T: Allocator> allocator_api2::alloc::Allocator for AllocWrapper<T> {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            Allocator::allocate(&self.0, layout)
        }

        #[inline]
        fn allocate_zeroed(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            Allocator::allocate_zeroed(&self.0, layout)
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::deallocate(&self.0, ptr, layout) }
        }

        #[inline]
        unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::grow(&self.0, ptr, old_layout, new_layout) }
        }

        #[inline]
        unsafe fn grow_zeroed(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe {
                Allocator::grow_zeroed(&self.0, ptr, old_layout, new_layout)
            }
        }

        #[inline]
        unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: Inherited from the wrapped allocator.
            unsafe { Allocator::shrink(&self.0, ptr, old_layout, new_layout) }
        }
    }
}
