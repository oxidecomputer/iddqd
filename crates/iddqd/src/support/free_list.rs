//! A lazily-allocated free list of `usize` indexes.
//!
//! See [`FreeList`] for the full picture. This type is an internal
//! primitive used by [`ItemSet`](super::item_set::ItemSet); it is kept
//! in its own module so the unsafe surface can be audited independently
//! of the surrounding collection logic.

use crate::errors::{TryReserveError, TryReserveErrorKind};
use allocator_api2::alloc::{Allocator, Layout};
use core::{
    marker::PhantomData,
    mem::{align_of, size_of},
    ptr::NonNull,
};

// `FreeListHeader::data_ptr` relies on the inline `usize` slots sitting
// flush against the header, i.e. no padding between them. That holds iff
// `size_of::<FreeListHeader>()` is a multiple of `align_of::<usize>()`,
// which is true for every target that has `#[repr(C)] struct { usize,
// usize }` layout.
//
// Assert this at compile time so a future platform or layout change can't
// silently break `data_ptr`.
const _: () = assert!(
    size_of::<FreeListHeader>() % align_of::<usize>() == 0,
    "FreeListHeader layout breaks the data_ptr invariant; \
     use the offset returned by `FreeListHeader::layout_for` instead",
);

/// A free list of `usize` indexes, lazily allocated.
///
/// The field is a single nullable pointer (`None` when no allocation has
/// happened yet), so it is one word on the stack. When an allocation is
/// present, it is laid out as a [`FreeListHeader`] followed inline by `cap`
/// `usize` slots.
///
/// The allocator is passed in explicitly every time the list is mutated. This
/// allows us to not require that the allocator be `Clone`, and in case of a
/// non-ZST allocator like bumpalo, it means that we don't have to store a
/// second copy of it. The caller is responsible for passing in the same
/// allocator on every call (this is documented as a safety requirement).
pub(crate) struct FreeList<A> {
    ptr: Option<NonNull<FreeListHeader>>,
    // Propagate auto traits as if we held an `A`.
    _marker: PhantomData<A>,
}

/// Header at the start of the free-list allocation.
///
/// Indexes live inline after this struct. This is opaque to callers outside
/// this module — it's `pub(crate)` only so [`FreeList::try_reserve_total`] can
/// mention `NonNull<FreeListHeader>` in its signature.
#[repr(C)]
pub(crate) struct FreeListHeader {
    len: usize,
    cap: usize,
}

impl FreeListHeader {
    /// Returns the allocation layout for a header followed by `cap` inline
    /// `usize` slots, plus the byte offset at which the slots begin.
    #[inline]
    fn layout_for(cap: usize) -> (Layout, usize) {
        Self::layout_for_checked(cap)
            .expect("free-list layout did not overflow")
    }

    /// Fallible variant of [`layout_for`] that surfaces capacity /
    /// layout overflow as `TryReserveError::CapacityOverflow` rather
    /// than panicking.
    #[inline]
    fn layout_for_checked(
        cap: usize,
    ) -> Result<(Layout, usize), TryReserveError> {
        let overflow = || {
            TryReserveError::__from_kind(TryReserveErrorKind::CapacityOverflow)
        };
        let header = Layout::new::<FreeListHeader>();
        let data = Layout::array::<usize>(cap).map_err(|_| overflow())?;
        let (combined, offset) = header.extend(data).map_err(|_| overflow())?;
        Ok((combined.pad_to_align(), offset))
    }

    /// Pointer to the inline slot array.
    ///
    /// # Safety
    ///
    /// `ptr` must have been produced by an allocation using
    /// [`Self::layout_for`].
    #[inline]
    unsafe fn data_ptr(ptr: NonNull<FreeListHeader>) -> *mut usize {
        // For `{usize, usize}`, `size_of::<FreeListHeader>()` is already
        // a multiple of `align_of::<usize>()`, so the first slot sits
        // flush against the header.
        (ptr.as_ptr() as *mut u8).add(size_of::<FreeListHeader>()) as *mut usize
    }
}

impl<A> FreeList<A> {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self { ptr: None, _marker: PhantomData }
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[usize] {
        match self.ptr {
            Some(ptr) => {
                // SAFETY: `ptr` was allocated by `ensure_capacity`, so the
                // header is initialized and the inline slots up to `len`
                // are initialized.
                unsafe {
                    let header = ptr.as_ref();
                    let data = FreeListHeader::data_ptr(ptr);
                    core::slice::from_raw_parts(data, header.len)
                }
            }
            None => &[],
        }
    }

    #[inline]
    pub(crate) fn last(&self) -> Option<usize> {
        self.as_slice().last().copied()
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<usize> {
        let mut ptr = self.ptr?;
        // SAFETY: `ptr` is live (since self.ptr was Some) and we have
        // unique access via `&mut self`.
        //
        // There is a subtle detail here: header (a &mut FreeListHeader) is
        // alive until the end of the function. The scope of the mutable borrow
        // is the header. value points to data after the header, so there's no
        // aliasing of mutable data.
        let header = unsafe { ptr.as_mut() };
        if header.len == 0 {
            return None;
        }
        header.len -= 1;
        // SAFETY: `header.len` was just decremented to an in-bounds
        // index, and that slot was initialized when pushed.
        let value =
            unsafe { FreeListHeader::data_ptr(ptr).add(header.len).read() };
        Some(value)
    }

    /// Pushes `value`, growing the backing allocation if needed.
    ///
    /// # Safety
    ///
    /// `alloc` must be the same allocator (or a functionally equivalent
    /// handle) that was used for every prior mutation of this free list.
    #[inline]
    pub(crate) unsafe fn push<T: Allocator>(
        &mut self,
        value: usize,
        alloc: &T,
    ) {
        // SAFETY: additional (= 1) is greater than 0; the allocator contract is
        // forwarded.
        let mut ptr = unsafe { self.ensure_capacity(1, alloc) };
        // SAFETY: `ptr` is the header returned by `ensure_capacity`,
        // which guarantees room for at least `header.len + 1` slots.
        unsafe {
            let header = ptr.as_mut();
            FreeListHeader::data_ptr(ptr).add(header.len).write(value);
            header.len += 1;
        }
    }

    /// Zeros the stored length without deallocating. Preserves capacity
    /// so a prior `try_reserve_total` reservation survives.
    #[inline]
    pub(crate) fn clear(&mut self) {
        if let Some(mut ptr) = self.ptr {
            // SAFETY: `ptr` is live and we hold `&mut self`, so no other
            // reference into the header exists.
            unsafe { ptr.as_mut().len = 0 };
        }
    }

    /// Deallocates any backing storage.
    pub(crate) fn deallocate<T: Allocator>(&mut self, alloc: &T) {
        let Some(ptr) = self.ptr.take() else { return };
        // SAFETY: this block was allocated via `alloc` (by the contract of
        // `push`) using a layout computed from the recorded capacity, so
        // it is sound to deallocate with the same allocator and layout.
        unsafe {
            let cap = ptr.as_ref().cap;
            let (layout, _) = FreeListHeader::layout_for(cap);
            alloc.deallocate(ptr.cast::<u8>(), layout);
        }
    }

    /// Current allocated capacity. Zero if the free list has never
    /// allocated.
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        match self.ptr {
            // SAFETY: `ptr` is live; the header was initialized at
            // allocation time and only mutated through `&mut self`.
            Some(ptr) => unsafe { ptr.as_ref().cap },
            None => 0,
        }
    }

    /// Fallibly grows capacity so that `self.capacity() >= total`, returning a
    /// pointer to the (possibly-reallocated) header.
    ///
    /// Growth is amortized: on a reallocation, the new capacity is the maximum
    /// of `total`, twice the current capacity, and 4 (the floor for a first
    /// allocation).
    ///
    /// Used by
    /// [`ItemSet::try_reserve`](super::item_set::ItemSet::try_reserve)
    /// to front-load the allocation that
    /// [`ItemSet::remove`](super::item_set::ItemSet::remove) would
    /// otherwise do lazily. The infallible variant
    /// [`ensure_capacity`](Self::ensure_capacity) is a thin wrapper
    /// that translates allocator failure into a panic, for the
    /// insert/clone/remove call sites whose callers already accept
    /// OOM abort.
    ///
    /// # Safety
    ///
    /// - The caller must pass the same allocator on every call for a
    ///   given `FreeList`.
    /// - `total` must be nonzero. Under that contract the returned
    ///   pointer is always live.
    pub(crate) unsafe fn try_reserve_total<T: Allocator>(
        &mut self,
        total: usize,
        alloc: &T,
    ) -> Result<NonNull<FreeListHeader>, TryReserveError> {
        let current_cap = self.capacity();
        if current_cap >= total {
            // SAFETY: caller guarantees `total > 0`, so
            // `current_cap >= total > 0` implies `self.ptr` is `Some`.
            return Ok(unsafe { self.ptr.unwrap_unchecked() });
        }
        // Amortized growth: at least double the existing capacity,
        // with a floor of 4 on a first allocation. Matches
        // `RawVec::grow_amortized`.
        let new_cap = total.max(current_cap.saturating_mul(2)).max(4);
        let (layout, _) = FreeListHeader::layout_for_checked(new_cap)?;
        let new_mem = alloc.allocate(layout).map_err(|_| {
            TryReserveError::__from_kind(TryReserveErrorKind::AllocError {
                layout,
            })
        })?;
        let new_ptr = new_mem.cast::<FreeListHeader>();
        // Copy existing contents (if any), then swap in the new block
        // and free the old one.
        //
        // SAFETY: `new_ptr` points at a fresh allocation sized by
        // `layout`, so writing the header is in-bounds. If there is an
        // old allocation, its layout is recorded in `old_cap` (read
        // before the deallocation) and it came from the same allocator
        // by the contract of `try_reserve_total` / `ensure_capacity`.
        unsafe {
            match self.ptr {
                Some(old_ptr) => {
                    let old_len;
                    let old_cap;
                    {
                        let header = old_ptr.as_ref();
                        old_len = header.len;
                        old_cap = header.cap;
                    }
                    new_ptr
                        .as_ptr()
                        .write(FreeListHeader { len: old_len, cap: new_cap });
                    let old_data = FreeListHeader::data_ptr(old_ptr);
                    let new_data = FreeListHeader::data_ptr(new_ptr);
                    core::ptr::copy_nonoverlapping(old_data, new_data, old_len);
                    let (old_layout, _) = FreeListHeader::layout_for(old_cap);
                    alloc.deallocate(old_ptr.cast::<u8>(), old_layout);
                }
                None => {
                    new_ptr
                        .as_ptr()
                        .write(FreeListHeader { len: 0, cap: new_cap });
                }
            }
        }
        self.ptr = Some(new_ptr);
        Ok(new_ptr)
    }

    /// Length of the allocated contents.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        match self.ptr {
            // SAFETY: `ptr` is live; the header was initialized at
            // allocation time.
            Some(ptr) => unsafe { ptr.as_ref().len },
            None => 0,
        }
    }

    /// Ensures there is room for at least `additional` more slots and
    /// returns a pointer to the (possibly-reallocated) header.
    ///
    /// # Safety
    ///
    /// - The caller must pass the same allocator on every call for a
    ///   given `FreeList`.
    /// - `additional` must be nonzero. Under that contract, the returned
    ///   pointer is always live.
    unsafe fn ensure_capacity<T: Allocator>(
        &mut self,
        additional: usize,
        alloc: &T,
    ) -> NonNull<FreeListHeader> {
        let needed = self
            .len()
            .checked_add(additional)
            .expect("free-list capacity did not overflow");
        // SAFETY: caller guarantees `additional > 0`, so `needed > 0`
        // satisfies `try_reserve_total`'s nonzero precondition.
        // Allocator contract forwarded.
        match unsafe { self.try_reserve_total(needed, alloc) } {
            Ok(ptr) => ptr,
            Err(e) => match *e.kind() {
                TryReserveErrorKind::AllocError { layout } => {
                    alloc::alloc::handle_alloc_error(layout)
                }
                TryReserveErrorKind::CapacityOverflow => {
                    panic!(
                        "free-list capacity overflow: needed={needed} \
                         exceeds Layout::array::<usize> bounds"
                    )
                }
            },
        }
    }
}

// SAFETY: `FreeList<A>` logically owns a `Box<[usize]>` plus the phantom
// allocator `A` that the enclosing `ItemSet` uses to allocate. The raw pointer
// disables the compiler's auto-trait derivation, so we re-state the conclusion
// the compiler would otherwise reach: `FreeList<A>` is `Send` if `A` is `Send`.
unsafe impl<A: Send> Send for FreeList<A> {}
// SAFETY: See the `Send` impl above. Similarly, `FreeList<A>` is `Sync` if `A`
// is `Sync`.
unsafe impl<A: Sync> Sync for FreeList<A> {}
