pub use self::inner::Global;
pub(crate) use self::inner::{AllocWrapper, Allocator, global_alloc};

// Basic non-nightly case.
// This uses `allocator-api2` enabled by default.
// If any crate enables "nightly" in `allocator-api2`,
// this will be equivalent to the nightly case,
// since `allocator_api2::alloc::Allocator` would be re-export of
// `core::alloc::Allocator`.
#[cfg(feature = "allocator-api2")]
mod inner {
    use allocator_api2::alloc::AllocError;
    pub use allocator_api2::alloc::{Allocator, Global, Layout};
    use core::ptr::NonNull;

    #[inline]
    pub(crate) fn global_alloc() -> Global {
        Global
    }

    #[derive(Clone, Copy, Default)]
    pub(crate) struct AllocWrapper<T>(pub(crate) T);

    unsafe impl<T: Allocator> allocator_api2::alloc::Allocator for AllocWrapper<T> {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            allocator_api2::alloc::Allocator::allocate(&self.0, layout)
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            allocator_api2::alloc::Allocator::deallocate(&self.0, ptr, layout);
        }
    }
}

// No-defaults case.
// When building with default-features turned off and
// neither `nightly` nor `allocator-api2` is enabled,
// this will be used.
// Making it impossible to use any custom allocator with collections defined
// in this crate.
// Any crate in build-tree can enable `allocator-api2`,
// or `nightly` without disturbing users that don't want to use it.
#[cfg(not(feature = "allocator-api2"))]
mod inner {
    use crate::alloc::alloc::Layout;
    use allocator_api2::alloc::AllocError;
    use core::ptr::NonNull;

    #[inline]
    pub(crate) fn global_alloc() -> Global {
        Global::default()
    }

    #[allow(clippy::missing_safety_doc)] // not exposed outside of this crate
    pub unsafe trait Allocator {
        fn allocate(&self, layout: Layout)
        -> Result<NonNull<[u8]>, AllocError>;
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
    }

    #[derive(Copy, Clone, Default)]
    #[doc(hidden)]
    pub struct Global(allocator_api2::alloc::Global);

    unsafe impl Allocator for Global {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            allocator_api2::alloc::Allocator::allocate(&self.0, layout)
        }

        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            allocator_api2::alloc::Allocator::deallocate(&self.0, ptr, layout);
        }
    }

    #[derive(Clone, Copy, Default)]
    pub(crate) struct AllocWrapper<T>(pub(crate) T);

    unsafe impl<T: Allocator> allocator_api2::alloc::Allocator for AllocWrapper<T> {
        #[inline]
        fn allocate(
            &self,
            layout: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            Allocator::allocate(&self.0, layout)
        }
        #[inline]
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            Allocator::deallocate(&self.0, ptr, layout);
        }
    }
}
