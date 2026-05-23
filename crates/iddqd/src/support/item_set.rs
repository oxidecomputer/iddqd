//! A dense, index-keyed container for items.
//!
//! # Design
//!
//! Each slot is an `ItemSlot<T>` that is either `Occupied(T)` or `Vacant {
//! next }`. The free chain consists of vacant slots that are linked together
//! via `next` pointers, with `free_head` as its LIFO top and
//! [`ItemIndex::SENTINEL`] as the end-of-list sentinel.
//!
//! Removed slots are recycled by the next [`GrowHandle::insert`], so a churn
//! workload stabilizes at the high-water mark of simultaneously-live items.
//!
//! The container maintains a single allocation (`items`) and uses two `u32`s
//! of stack footprint beyond it (the `free_head` and the current `len`).
//!
//! # Why slot-based
//!
//! We also tried a `Vec<Option<T>>` plus a separately allocated free list for
//! vacant indexes. That was optimal storage for any `T` with a niche
//! (`size_of::<Option<T>>() == size_of::<T>()`). But this came at the cost of a
//! hand-rolled unsafe allocator to manage the secondary allocation (i.e., north
//! of 350 lines of layout-math, lifetime, and `Send`/`Sync` reasoning). The
//! slot-based layout eliminates that module entirely: the only unsafe in this
//! file is the disjoint-indexes trick in `get_disjoint_mut`, which any
//! slot-based container needs regardless of backend.
//!
//! The tradeoff is that `ItemSlot<T>` carries a discriminant, so slots are at
//! least `max(size_of::<u32>(), size_of::<T>()) + align_of::<ItemSlot<T>>()`.
//! For types with a niche (including structs where a field has a niche), this
//! is one word larger per slot than `Option<T>` would be. Benchmarking
//! indicates that overall this is a wash. Based on that, we choose the
//! implementation with less unsafe code.
//!
//! # Invariants
//!
//! 1. For every `i < items.len()`: `items[i]` is `Occupied` iff `i` is
//!    not reachable from `free_head` via the `Vacant::next` chain.
//! 2. The `Vacant::next` chain starting at `free_head` terminates at
//!    `SENTINEL`, visits every vacant slot exactly once, and
//!    contains no in-bounds index that refers to an `Occupied` slot.
//! 3. `len == items.iter().filter(|e| matches!(e, Occupied(_))).count()`.
//!
//! Under these invariants:
//!
//! * The number of occupied slots is `self.len`.
//! * The number of vacant slots is `items.len() - self.len`.

use super::{
    ItemIndex,
    alloc::{AllocWrapper, Allocator, Global, global_alloc},
};
use crate::{
    errors::TryReserveError,
    internal::{ValidateCompact, ValidationError},
};
use allocator_api2::vec::Vec;
use core::{
    fmt,
    iter::FusedIterator,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

/// A remap from old (pre-compaction) to new (post-compaction) indexes.
///
/// Produced by [`ItemSet::compact`], and consumed by the outer tables (hash /
/// btree index tables) so they can rewrite their stored indexes to point at
/// the compacted `items` buffer. The capacity shrink that typically follows
/// is a separate step so that an allocator panic during the
/// shrink cannot leave the tables pointing at pre-compaction indexes.
///
/// Two cases:
///
/// - [`IndexRemap::Identity`]: compaction was a no-op (no holes were
///   filled), so every old index is still valid as-is.
/// - [`IndexRemap::Permuted`]: holes were filled. The contained
///   `Vec<ItemIndex>` is a direct position array — `new_pos[old]` is
///   the new index, or [`ItemIndex::SENTINEL`] for slots that were vacated.
#[derive(Clone, Debug)]
pub(crate) enum IndexRemap {
    /// Compaction was a no-op: every old slot index is still valid.
    Identity,
    /// Slots moved during compaction. `new_pos[old]` is either the new index
    /// for the slot that used to live at `old`, or [`ItemIndex::SENTINEL`] if
    /// `old` was vacant at compaction time.
    Permuted(alloc::vec::Vec<ItemIndex>),
}

impl IndexRemap {
    #[inline]
    pub(crate) fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }

    /// Looks up the post-compaction index for `old`.
    ///
    /// Panics if `old` was a slot that compaction vacated. This indicates a
    /// caller bug: those indexes should already have been removed from the
    /// outer index before [`ItemSet::compact`] was called.
    #[inline]
    pub(crate) fn remap(&self, old: ItemIndex) -> ItemIndex {
        match self {
            Self::Identity => old,
            Self::Permuted(new_pos) => {
                let new = new_pos[old.as_u32() as usize];
                if new == ItemIndex::SENTINEL {
                    panic!(
                        "IndexRemap::remap called on a compacted-away \
                         index {old}"
                    )
                }
                new
            }
        }
    }
}

/// A typestate that proves there's space within the item set to grow the set by
/// exactly one slot.
///
/// This handle is created by [`ItemSet::assert_can_grow`] and consumed by
/// [`GrowHandle::insert`]. The handle holds a `&mut ItemSet`.
///
/// Splitting the assertion from the insertion lets callers fail the cap check
/// before indexes are mutated. During this interval, if callers need access to
/// the individual items, they can use the `Index<ItemIndex>` impl below. More
/// functionality can be added to this handle as necessary.
#[must_use = "must be consumed by GrowHandle::insert"]
pub(crate) struct GrowHandle<'a, T, A: Allocator> {
    items: &'a mut ItemSet<T, A>,
}

impl<T, A: Allocator> core::ops::Deref for GrowHandle<'_, T, A> {
    type Target = ItemSet<T, A>;

    #[inline]
    fn deref(&self) -> &ItemSet<T, A> {
        self.items
    }
}

impl<'a, T, A: Allocator> GrowHandle<'a, T, A> {
    /// Returns the index that [`Self::insert`] will assign.
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    #[inline]
    pub(crate) fn next_index(&self) -> ItemIndex {
        if self.free_head == ItemIndex::SENTINEL {
            // `assert_can_grow` enforces `items.len() <= ItemIndex::MAX_VALID`,
            // so this conversion is lossless.
            ItemIndex::new(self.items.len() as u32)
        } else {
            // Use the LIFO slot.
            self.free_head
        }
    }

    /// Inserts `value` at [`Self::next_index`] and returns the chosen
    /// index, consuming the handle.
    ///
    /// This is the only way to grow an [`ItemSet`].
    #[inline]
    pub(crate) fn insert(self, value: T) -> ItemIndex {
        if self.items.free_head == ItemIndex::SENTINEL {
            // `assert_can_grow` guarantees `items.len() <= ItemIndex::MAX_VALID`,
            // so this u32 conversion cannot lose precision.
            let idx = ItemIndex::new(self.items.items.len() as u32);
            self.items.items.push(ItemSlot::Occupied(value));
            self.items.len += 1;
            idx
        } else {
            let idx = self.items.free_head;
            // Replace the `Vacant { next }` at `idx` with `Occupied`,
            // and advance `free_head` to `next`.
            let slot = &mut self.items.items[idx.as_u32() as usize];
            let next = match slot {
                ItemSlot::Occupied(_) => {
                    panic!("ItemSet free chain points at occupied slot {idx}")
                }
                ItemSlot::Vacant { next } => *next,
            };
            *slot = ItemSlot::Occupied(value);
            self.items.free_head = next;
            self.items.len += 1;
            idx
        }
    }
}

/// A single slot in an [`ItemSet`].
///
/// Exposed at `pub(crate)` because [`ItemSet::slots_mut`] hands out a slot
/// slice for `ItemSlotsPtr` to build an allocator-agnostic raw pointer over.
/// Callers need to name the element type. All other interaction with slots
/// goes through `ItemSet`'s safe methods.
#[derive(Clone, Debug)]
pub(crate) enum ItemSlot<T> {
    /// The slot holds a live value.
    Occupied(T),
    /// The slot is free.
    ///
    /// `next` is the index of the next free slot, or [`ItemIndex::SENTINEL`] if
    /// this is the end of the chain.
    Vacant { next: ItemIndex },
}

impl<T> ItemSlot<T> {
    /// Returns a reference to the contained value, if occupied.
    #[inline]
    fn as_ref(&self) -> Option<&T> {
        match self {
            ItemSlot::Occupied(v) => Some(v),
            ItemSlot::Vacant { .. } => None,
        }
    }

    /// Returns a mutable reference to the contained value, if occupied.
    #[inline]
    pub(crate) fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            ItemSlot::Occupied(v) => Some(v),
            ItemSlot::Vacant { .. } => None,
        }
    }

    #[inline]
    fn is_occupied(&self) -> bool {
        match self {
            ItemSlot::Occupied(_) => true,
            ItemSlot::Vacant { .. } => false,
        }
    }
}

/// A dense, index-keyed container for items.
///
/// See the [module-level docs](self) for the design and tradeoffs.
pub(crate) struct ItemSet<T, A: Allocator> {
    items: Vec<ItemSlot<T>, AllocWrapper<A>>,
    /// LIFO head of the embedded free chain, or [`ItemIndex::SENTINEL`] when no
    /// slots are free.
    free_head: ItemIndex,
    /// Count of `Occupied` slots, maintained by insert/remove.
    ///
    /// (ItemIndex is a u32, as is len, so the struct can be more tightly packed
    /// than if both were usizes.)
    len: u32,
}

impl<T: Clone, A: Clone + Allocator> Clone for ItemSet<T, A> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            free_head: self.free_head,
            len: self.len,
        }
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for ItemSet<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemSet")
            .field("len", &self.len)
            .field("slots", &self.items.len())
            .field("free_head", &self.free_head)
            .finish()
    }
}

impl<T> ItemSet<T, Global> {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            items: Vec::new_in(AllocWrapper(global_alloc())),
            free_head: ItemIndex::SENTINEL,
            len: 0,
        }
    }
}

impl<T, A: Allocator> ItemSet<T, A> {
    #[inline]
    pub(crate) const fn new_in(alloc: A) -> Self {
        Self {
            items: Vec::new_in(AllocWrapper(alloc)),
            free_head: ItemIndex::SENTINEL,
            len: 0,
        }
    }

    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: Vec::with_capacity_in(capacity, AllocWrapper(alloc)),
            free_head: ItemIndex::SENTINEL,
            len: 0,
        }
    }

    pub(crate) fn allocator(&self) -> &A {
        &self.items.allocator().0
    }

    /// Returns the backing slot buffer as a mutable slice.
    ///
    /// Used by [`ItemSlotsPtr::new`] to build an allocator-agnostic raw
    /// pointer over the slot buffer for the per-map `IterMut` iterators.
    #[inline]
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    pub(crate) fn slots_mut(&mut self) -> &mut [ItemSlot<T>] {
        &mut self.items
    }

    pub(crate) fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        let occupied_count =
            self.items.iter().filter(|e| e.is_occupied()).count();
        if occupied_count != self.len as usize {
            return Err(ValidationError::General(format!(
                "ItemSet len ({}) disagrees with occupied-slot count ({})",
                self.len, occupied_count,
            )));
        }

        // Walk the free chain and verify the following properties:
        //
        // * Every visited index is in bounds.
        // * Every visited slot is `Vacant`.
        // * We visit exactly `items.len() - len` slots (i.e. each
        //   vacant slot exactly once); this detects cycles and missing
        //   links at the same time.
        let Some(expected_vacant) =
            self.items.len().checked_sub(self.len as usize)
        else {
            return Err(ValidationError::General(format!(
                "ItemSet len ({}) exceeds items.len() ({})",
                self.len,
                self.items.len(),
            )));
        };

        let mut walked = 0usize;
        let mut cursor = self.free_head;
        while cursor != ItemIndex::SENTINEL {
            let cursor_idx = cursor.as_u32() as usize;
            if cursor_idx >= self.items.len() {
                return Err(ValidationError::General(format!(
                    "ItemSet free chain has out-of-range index {cursor}"
                )));
            }
            match &self.items[cursor_idx] {
                ItemSlot::Occupied(_) => {
                    return Err(ValidationError::General(format!(
                        "ItemSet free chain points at occupied slot \
                         {cursor}"
                    )));
                }
                ItemSlot::Vacant { next } => {
                    walked += 1;
                    if walked > expected_vacant {
                        return Err(ValidationError::General(format!(
                            "ItemSet free chain cycles or overshoots: \
                             walked {walked} vacant slots, expected \
                             {expected_vacant}"
                        )));
                    }
                    cursor = *next;
                }
            }
        }
        if walked != expected_vacant {
            return Err(ValidationError::General(format!(
                "ItemSet free chain length {walked} disagrees with \
                 vacant-slot count {expected_vacant}"
            )));
        }

        match compactness {
            ValidateCompact::Compact => {
                if expected_vacant != 0 {
                    return Err(ValidationError::General(format!(
                        "ItemSet is not compact: {expected_vacant} \
                         vacant slots",
                    )));
                }
            }
            ValidateCompact::NonCompact => {}
        }

        Ok(())
    }

    pub(crate) fn capacity(&self) -> usize {
        self.items.capacity()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub(crate) fn iter(&self) -> Iter<'_, T> {
        Iter::new(self)
    }

    #[inline]
    #[expect(dead_code)]
    pub(crate) fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(self)
    }

    #[inline]
    pub(crate) fn values(&self) -> Values<'_, T> {
        Values::new(self)
    }

    #[inline]
    pub(crate) fn values_mut(&mut self) -> ValuesMut<'_, T> {
        ValuesMut::new(self)
    }

    #[inline]
    pub(crate) fn into_values(self) -> IntoValues<T, A> {
        IntoValues::new(self)
    }

    #[inline]
    pub(crate) fn get(&self, index: ItemIndex) -> Option<&T> {
        self.items.get(index.as_u32() as usize).and_then(ItemSlot::as_ref)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: ItemIndex) -> Option<&mut T> {
        self.items.get_mut(index.as_u32() as usize).and_then(ItemSlot::as_mut)
    }

    /// Returns mutable references to up to `N` distinct indexes.
    ///
    /// Returns `None` for any index that is out of bounds, vacant, or
    /// that duplicates an earlier index in the array.
    pub(crate) fn get_disjoint_mut<const N: usize>(
        &mut self,
        indexes: [&ItemIndex; N],
    ) -> [Option<&mut T>; N] {
        let len = self.items.len();
        let mut valid = [false; N];
        for i in 0..N {
            let idx = indexes[i].as_u32() as usize;
            if idx >= len {
                continue;
            }
            // SAFETY: idx < len, so `items[idx]` is in bounds.
            if !unsafe { self.items.get_unchecked(idx) }.is_occupied() {
                continue;
            }
            let mut dup = false;
            for j in 0..i {
                if valid[j] && indexes[j].as_u32() == indexes[i].as_u32() {
                    dup = true;
                    break;
                }
            }
            if !dup {
                valid[i] = true;
            }
        }

        let base = self.items.as_mut_ptr();
        core::array::from_fn(|i| {
            if valid[i] {
                let idx = indexes[i].as_u32() as usize;
                // SAFETY: we verified idx is in bounds, the slot is
                // `Occupied`, and no earlier valid entry shares this
                // index. Therefore the `&mut` references are disjoint.
                unsafe { (*base.add(idx)).as_mut() }
            } else {
                None
            }
        })
    }

    /// Returns a [`GrowHandle`] that grants exclusive access to grow the set by
    /// exactly one slot, panicking if the set is already full.
    ///
    /// The returned handle is the only way to grow an [`ItemSet`], so the
    /// capacity check cannot be skipped. Because the handle holds a `&mut
    /// ItemSet`, the item set cannot be mutated in between.
    #[inline]
    #[must_use = "GrowHandle must be passed to GrowHandle::insert"]
    pub(crate) fn assert_can_grow(&mut self) -> GrowHandle<'_, T, A> {
        if self.free_head == ItemIndex::SENTINEL {
            assert!(
                self.items.len() <= ItemIndex::MAX_VALID.as_u32() as usize,
                "ItemSet index exceeds maximum index {}",
                ItemIndex::MAX_VALID,
            );
        } else {
            // At least one vacant slot is available in self.items.
        }
        GrowHandle { items: self }
    }

    /// Removes the item at `index`, if any.
    ///
    /// This does not allocate: the freed index threads onto the embedded chain
    /// in place.
    ///
    /// `items` is not truncated here, even for a trailing remove. The vacated
    /// slot stays in place until reused by the next insert or reclaimed by
    /// [`compact`](Self::compact).
    #[inline]
    pub(crate) fn remove(&mut self, index: ItemIndex) -> Option<T> {
        let slot = self.items.get_mut(index.as_u32() as usize)?;
        if !slot.is_occupied() {
            return None;
        }
        let ItemSlot::Occupied(v) =
            core::mem::replace(slot, ItemSlot::Vacant { next: self.free_head })
        else {
            unreachable!("is_occupied was just checked")
        };
        self.free_head = index;
        self.len = self.len.checked_sub(1).expect("ItemSet len should be > 0");
        Some(v)
    }

    /// Consumes this set into an owned, invariant-free
    /// [`ConsumingItemSet`].
    pub(crate) fn into_consuming(self) -> ConsumingItemSet<T, A> {
        ConsumingItemSet { items: self.items }
    }

    /// Clears the item set, removing all items.
    ///
    /// Preserves `items.capacity()`, matching the behavior of
    /// [`Vec::clear`]. Any prior [`try_reserve`](Self::try_reserve)
    /// reservation survives a `clear`.
    pub(crate) fn clear(&mut self) {
        // Publish the post-clear metadata before dropping items, so a user
        // `Drop` panic cannot leave len/free_head describing the old slots.
        self.free_head = ItemIndex::SENTINEL;
        self.len = 0;
        self.items.clear();
    }

    /// This method assumes that value has the same ID. It also asserts
    /// that `index` is valid (and panics if it isn't).
    #[inline]
    pub(crate) fn replace(&mut self, index: ItemIndex, value: T) -> T {
        let Some(slot) = self
            .items
            .get_mut(index.as_u32() as usize)
            .filter(|s| s.is_occupied())
        else {
            panic!("ItemSet index not found: {index}")
        };
        let ItemSlot::Occupied(old) =
            core::mem::replace(slot, ItemSlot::Occupied(value))
        else {
            unreachable!("slot was just matched as Occupied")
        };
        old
    }

    #[inline]
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
    }

    /// Shrinks the backing buffer's capacity to fit the current length.
    ///
    /// Must be called *after* [`compact`](Self::compact) has been run and
    /// the outer tables have been remapped via the returned [`IndexRemap`].
    /// Splitting capacity-shrinking out of compaction means that an
    /// allocator panic in this call cannot leave the tables pointing at
    /// pre-compaction indexes (by the time we get here, the tables and
    /// `items` are already in sync).
    #[inline]
    pub(crate) fn shrink_capacity_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Shrinks the backing buffer's capacity, leaving at least `min_capacity`
    /// slots reserved.
    ///
    /// See [`shrink_capacity_to_fit`](Self::shrink_capacity_to_fit) for the
    /// panic-safety rationale for splitting this from compaction.
    #[inline]
    pub(crate) fn shrink_capacity_to(&mut self, min_capacity: usize) {
        self.items.shrink_to(min_capacity);
    }

    /// Moves every live slot down to fill `Vacant` holes, truncates
    /// `items` to its new length, and clears the free chain.
    ///
    /// Does *not* shrink the underlying buffer's capacity. Callers should
    /// remap any externally-stored indexes via the returned [`IndexRemap`]
    /// before calling [`shrink_capacity_to_fit`](Self::shrink_capacity_to_fit)
    /// or [`shrink_capacity_to`](Self::shrink_capacity_to).
    pub(crate) fn compact(&mut self) -> IndexRemap {
        let pre_len = self.items.len();
        if pre_len == self.len as usize {
            // Already compact, so there's nothing to remap.
            debug_assert_eq!(
                self.free_head,
                ItemIndex::SENTINEL,
                "compact: items full but free_head not SENTINEL ({})",
                self.free_head,
            );
            return IndexRemap::Identity;
        }

        // Do a forward scan, writing each `Occupied` into the next write
        // position. As we go, build a `new_pos[old] = new` index so callers can
        // rewrite their stored indexes.
        assert!(
            pre_len <= ItemIndex::MAX_VALID.as_u32() as usize,
            "compact: items.len() {pre_len} exceeds MAX_VALID {}",
            ItemIndex::MAX_VALID,
        );
        let mut new_pos: alloc::vec::Vec<ItemIndex> =
            alloc::vec::Vec::with_capacity(pre_len);
        let mut write: u32 = 0;
        for read in 0..pre_len {
            match &self.items[read] {
                ItemSlot::Occupied(_) => {
                    new_pos.push(ItemIndex::new(write));
                    if write as usize != read {
                        self.items.swap(write as usize, read);
                    }
                    write += 1;
                }
                ItemSlot::Vacant { .. } => {
                    new_pos.push(ItemIndex::SENTINEL);
                }
            }
        }
        self.items.truncate(write as usize);
        self.free_head = ItemIndex::SENTINEL;
        // `len` is unchanged: we truncated only `Vacant` entries.

        IndexRemap::Permuted(new_pos)
    }

    /// Tries to reserve capacity for at least `additional` more items.
    ///
    /// After this call returns `Ok(())`, the next `additional` calls
    /// to [`GrowHandle::insert`] are OOM-free. `remove` is always
    /// OOM-free regardless.
    #[inline]
    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), TryReserveError> {
        self.items
            .try_reserve(additional)
            .map_err(TryReserveError::from_allocator_api2)
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::ItemSet;
    use crate::support::alloc::Allocator;
    use serde_core::{Serialize, Serializer};

    impl<T: Serialize, A: Allocator> Serialize for ItemSet<T, A> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            // Serialize just the items -- don't serialize the map keys.
            // We'll rebuild the map keys on deserialization.
            serializer.collect_seq(self.values())
        }
    }
}

impl<T, A: Allocator> Index<ItemIndex> for ItemSet<T, A> {
    type Output = T;

    #[inline]
    fn index(&self, index: ItemIndex) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}

impl<T, A: Allocator> IndexMut<ItemIndex> for ItemSet<T, A> {
    #[inline]
    fn index_mut(&mut self, index: ItemIndex) -> &mut Self::Output {
        self.get_mut(index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}

// --- ItemSlotsPtr ---------------------------------------------------------

/// A raw pointer into the start of an [`ItemSet`]'s slot buffer, with the same
/// thread-safety properties as `&'a mut [ItemSlot<T>]`.
///
/// This is used by iterators that yield `&mut T` references one at a time and
/// need to keep handing out distinct references for the duration of the borrow.
///
/// This is equivalent to reborrowing the slot slice each iteration and using
/// unsafe code for lifetime extension, but that would invalidate previously
/// yielded `&mut T` children under Stacked Borrows. By using a raw pointer, we
/// keep the original mutable borrow live for the full iteration while still
/// being able to hand out element references that outlive `&mut self`. (Note
/// that the lifetime extension approach is not rejected by Tree Borrows, which
/// indicates that it's probably sound. But it's nice for iddqd to pass both
/// Stacked and Tree Borrows.)
///
/// The only way to read a slot through this pointer is the `unsafe`
/// [`Self::get_mut`] method, which the caller is responsible for invoking
/// with each `index` at most once across the lifetime of `'a`.
pub(crate) struct ItemSlotsPtr<'a, T> {
    /// The pointer to the start of the slot buffer.
    start_ptr: *mut ItemSlot<T>,
    /// Number of slots in the backing buffer at construction time.
    slot_count: usize,
    /// Borrow the slot slice for `'a` so the raw pointer stays live, and so
    /// that variance and drop-check work the same as `&'a mut [ItemSlot<T>]`.
    /// This deliberately does not mention `ItemSet<T, A>` so the iterator's
    /// public surface stays allocator-agnostic.
    _marker: PhantomData<&'a mut [ItemSlot<T>]>,
}

impl<T> fmt::Debug for ItemSlotsPtr<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemSlotsPtr")
            .field("start_ptr", &self.start_ptr)
            .field("slot_count", &self.slot_count)
            .finish()
    }
}

// SAFETY: We treat the `*mut ItemSlot<T>` as the `&'a mut [ItemSlot<T>]` that
// the `PhantomData` already encodes. Auto-trait inference would give us
// `Send`/`Sync` for that slice under `T: Send` (resp. `T: Sync`); raw
// pointers don't carry the auto-traits, so we state the same bound here.
unsafe impl<T: Send> Send for ItemSlotsPtr<'_, T> {}
// SAFETY: see the `Send` impl above.
unsafe impl<T: Sync> Sync for ItemSlotsPtr<'_, T> {}

impl<'a, T> ItemSlotsPtr<'a, T> {
    /// Captures a raw pointer into the slot buffer.
    ///
    /// The returned handle borrows `slots` for `'a`.
    #[inline]
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    pub(crate) fn new(slots: &'a mut [ItemSlot<T>]) -> Self {
        Self {
            start_ptr: slots.as_mut_ptr(),
            slot_count: slots.len(),
            _marker: PhantomData,
        }
    }

    /// Returns a mutable reference to the item at `index`.
    ///
    /// The lifetime of the returned reference is the borrow `'a` captured at
    /// construction. This lets callers (typically iterators) hand the reference
    /// out and then call `get_mut` again for a different index without
    /// invalidating prior yields.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds for the captured slot buffer or
    /// if the slot is `Vacant`. Both indicate a stale or invalid index
    /// reached us from an outer index table: this is a programmer error,
    /// not undefined behavior.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that, across the lifetime of this
    /// [`ItemSlotsPtr`], each `index` value is passed to `get_mut` at most
    /// once. That is the only thing that keeps the returned `&mut T`
    /// references disjoint and aliasing-free.
    #[inline]
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    pub(crate) unsafe fn get_mut(&mut self, index: ItemIndex) -> &'a mut T {
        let raw_index = index.as_u32() as usize;
        // Belt-and-suspenders bounds check. The outer index tables only ever
        // store indexes that point at occupied slots, but we panic rather
        // than risk UB if a future change inadvertently breaks that invariant.
        assert!(
            raw_index < self.slot_count,
            "ItemSlotsPtr index {raw_index} should be in bounds \
             for slot count {}",
            self.slot_count,
        );
        // SAFETY:
        //
        // * `raw_index < self.slot_count`, so `self.start_ptr.add(raw_index)`
        //   is in-bounds for the original slot slice.
        // * `ItemSlotsPtr::new` mutably borrowed the slot slice for `'a`, so
        //   no other code can touch it for the duration.
        // * The caller's distinctness contract guarantees that we never hand
        //   out two `&mut T` references to the same slot.
        // * The `expect` below verifies that the slot is `Occupied`; an outer
        //   index table that points at a `Vacant` slot is a programmer error
        //   that we surface as a panic rather than UB.
        unsafe {
            (*self.start_ptr.add(raw_index))
                .as_mut()
                .expect("ItemSlotsPtr index points at an occupied slot")
        }
    }
}

// --- Iterators ----------------------------------------------------------

/// An iterator over `(index, &item)` pairs in an [`ItemSet`].
pub(crate) struct Iter<'a, T> {
    inner: core::iter::Enumerate<core::slice::Iter<'a, ItemSlot<T>>>,
    remaining: usize,
}

impl<'a, T> Iter<'a, T> {
    fn new<A: Allocator>(set: &'a ItemSet<T, A>) -> Self {
        Self { inner: set.items.iter().enumerate(), remaining: set.len() }
    }
}

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), remaining: self.remaining }
    }
}

impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iter").field("remaining", &self.remaining).finish()
    }
}

impl<T> Default for Iter<'_, T> {
    fn default() -> Self {
        let empty: &[ItemSlot<T>] = &[];
        Self { inner: empty.iter().enumerate(), remaining: 0 }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (ItemIndex, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.inner.by_ref() {
            if let ItemSlot::Occupied(v) = slot {
                debug_assert!(
                    self.remaining > 0,
                    "iterator yielded more items than ItemSet::len()",
                );
                self.remaining -= 1;
                return Some((ItemIndex::new(i as u32), v));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T> FusedIterator for Iter<'_, T> {}

/// An iterator over `(index, &mut item)` pairs in an [`ItemSet`].
pub(crate) struct IterMut<'a, T> {
    inner: core::iter::Enumerate<core::slice::IterMut<'a, ItemSlot<T>>>,
    remaining: usize,
}

impl<'a, T> IterMut<'a, T> {
    fn new<A: Allocator>(set: &'a mut ItemSet<T, A>) -> Self {
        let remaining = set.len();
        Self { inner: set.items.iter_mut().enumerate(), remaining }
    }
}

impl<T: fmt::Debug> fmt::Debug for IterMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IterMut").field("remaining", &self.remaining).finish()
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (ItemIndex, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.inner.by_ref() {
            if let ItemSlot::Occupied(v) = slot {
                debug_assert!(
                    self.remaining > 0,
                    "iterator yielded more items than ItemSet::len()",
                );
                self.remaining -= 1;
                return Some((ItemIndex::new(i as u32), v));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T> FusedIterator for IterMut<'_, T> {}

/// An iterator over `&item` references in an [`ItemSet`].
pub(crate) struct Values<'a, T> {
    inner: core::slice::Iter<'a, ItemSlot<T>>,
    remaining: usize,
}

impl<'a, T> Values<'a, T> {
    fn new<A: Allocator>(set: &'a ItemSet<T, A>) -> Self {
        Self { inner: set.items.iter(), remaining: set.len() }
    }
}

impl<T> Clone for Values<'_, T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), remaining: self.remaining }
    }
}

impl<T: fmt::Debug> fmt::Debug for Values<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Values").field("remaining", &self.remaining).finish()
    }
}

impl<T> Default for Values<'_, T> {
    fn default() -> Self {
        let empty: &[ItemSlot<T>] = &[];
        Self { inner: empty.iter(), remaining: 0 }
    }
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.inner.by_ref() {
            if let ItemSlot::Occupied(v) = slot {
                debug_assert!(
                    self.remaining > 0,
                    "iterator yielded more items than ItemSet::len()",
                );
                self.remaining -= 1;
                return Some(v);
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for Values<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T> FusedIterator for Values<'_, T> {}

/// An iterator over `&mut item` references in an [`ItemSet`].
pub(crate) struct ValuesMut<'a, T> {
    inner: core::slice::IterMut<'a, ItemSlot<T>>,
    remaining: usize,
}

impl<'a, T> ValuesMut<'a, T> {
    fn new<A: Allocator>(set: &'a mut ItemSet<T, A>) -> Self {
        let remaining = set.len();
        Self { inner: set.items.iter_mut(), remaining }
    }
}

impl<T: fmt::Debug> fmt::Debug for ValuesMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValuesMut").field("remaining", &self.remaining).finish()
    }
}

impl<'a, T> Iterator for ValuesMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.inner.by_ref() {
            if let ItemSlot::Occupied(v) = slot {
                debug_assert!(
                    self.remaining > 0,
                    "iterator yielded more items than ItemSet::len()",
                );
                self.remaining -= 1;
                return Some(v);
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for ValuesMut<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T> FusedIterator for ValuesMut<'_, T> {}

/// An owning iterator over the items in an [`ItemSet`].
pub(crate) struct IntoValues<T, A: Allocator> {
    inner: allocator_api2::vec::IntoIter<ItemSlot<T>, AllocWrapper<A>>,
    remaining: usize,
}

impl<T, A: Allocator> IntoValues<T, A> {
    fn new(set: ItemSet<T, A>) -> Self {
        let remaining = set.len();
        let consuming = set.into_consuming();
        Self { inner: consuming.items.into_iter(), remaining }
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for IntoValues<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoValues")
            .field("remaining", &self.remaining)
            .finish()
    }
}

impl<T, A: Allocator> Iterator for IntoValues<T, A> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.inner.by_ref() {
            if let ItemSlot::Occupied(v) = slot {
                debug_assert!(
                    self.remaining > 0,
                    "iterator yielded more items than ItemSet::len()",
                );
                self.remaining -= 1;
                return Some(v);
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, A: Allocator> ExactSizeIterator for IntoValues<T, A> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, A: Allocator> FusedIterator for IntoValues<T, A> {}

/// An [`ItemSet`] consumed into an owned, by-index take-only version.
///
/// Produced by [`ItemSet::into_consuming`]. The free chain is no longer
/// maintained from here on.
pub(crate) struct ConsumingItemSet<T, A: Allocator> {
    items: Vec<ItemSlot<T>, AllocWrapper<A>>,
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for ConsumingItemSet<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsumingItemSet")
            .field("slots", &self.items.len())
            .finish()
    }
}

impl<T, A: Allocator> ConsumingItemSet<T, A> {
    /// Takes the item at `index`, leaving a `Vacant { next: SENTINEL }`
    /// slot behind.
    ///
    /// Returns `None` if `index` is out of bounds or the slot has
    /// already been taken. O(1) regardless of position.
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    #[inline]
    pub(crate) fn take(&mut self, index: ItemIndex) -> Option<T> {
        let slot = self.items.get_mut(index.as_u32() as usize)?;
        if !slot.is_occupied() {
            return None;
        }
        // The free chain is no longer maintained in this view, so any
        // `next` value is fine. `SENTINEL` is a natural choice.
        let ItemSlot::Occupied(v) = core::mem::replace(
            slot,
            ItemSlot::Vacant { next: ItemIndex::SENTINEL },
        ) else {
            unreachable!("is_occupied was just checked")
        };
        Some(v)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::internal::ValidateCompact;

    fn ix(value: u32) -> ItemIndex {
        ItemIndex::new(value)
    }

    #[test]
    fn compact_fills_middle_holes() {
        let mut set = ItemSet::<u32, Global>::new();
        for i in 0..5 {
            set.assert_can_grow().insert(i * 10);
        }
        set.remove(ix(1)).expect("slot was occupied");
        set.remove(ix(3)).expect("slot was occupied");

        let remap = set.compact();
        set.shrink_capacity_to_fit();

        assert_eq!(set.len(), 3);
        set.validate(ValidateCompact::Compact).unwrap();
        assert_eq!(&[set[ix(0)], set[ix(1)], set[ix(2)]], &[0, 20, 40]);

        assert!(!remap.is_identity());
        assert_eq!(remap.remap(ix(0)), ix(0));
        assert_eq!(remap.remap(ix(2)), ix(1));
        assert_eq!(remap.remap(ix(4)), ix(2));
    }

    #[test]
    fn compact_without_holes_returns_identity_remap() {
        let mut set = ItemSet::<u32, Global>::new();
        for i in 0..4 {
            set.assert_can_grow().insert(i);
        }
        let remap = set.compact();
        set.shrink_capacity_to_fit();
        assert!(remap.is_identity());
        set.validate(ValidateCompact::Compact)
            .expect("a hole-free set is trivially compact after compact");
    }

    #[test]
    fn free_chain_is_lifo_and_well_formed() {
        let mut set = ItemSet::<u32, Global>::new();
        for i in 0..6 {
            set.assert_can_grow().insert(i * 10);
        }
        // Remove 1, then 3, then 5 — free chain after is [5 -> 3 -> 1].
        assert_eq!(set.remove(ix(1)), Some(10));
        assert_eq!(set.remove(ix(3)), Some(30));
        assert_eq!(set.remove(ix(5)), Some(50));
        set.validate(ValidateCompact::NonCompact).unwrap();
        assert_eq!(set.len(), 3);

        // LIFO reuse: next three inserts go into 5, 3, 1.
        assert_eq!(set.assert_can_grow().insert(100), ix(5));
        assert_eq!(set.assert_can_grow().insert(200), ix(3));
        assert_eq!(set.assert_can_grow().insert(300), ix(1));
        set.validate(ValidateCompact::Compact).unwrap();
        assert_eq!(set[ix(1)], 300);
        assert_eq!(set[ix(3)], 200);
        assert_eq!(set[ix(5)], 100);

        // Fourth insert allocates a new slot.
        assert_eq!(set.assert_can_grow().insert(400), ix(6));
    }

    #[test]
    fn clone_preserves_free_chain_and_values() {
        let mut set = ItemSet::<u32, Global>::new();
        for i in 0..4 {
            set.assert_can_grow().insert(i);
        }
        set.remove(ix(1));
        set.remove(ix(2));

        let cloned = set.clone();
        cloned.validate(ValidateCompact::NonCompact).unwrap();
        assert_eq!(cloned.len(), set.len());
        assert_eq!(cloned.get(ix(0)), Some(&0));
        assert_eq!(cloned.get(ix(1)), None);
        assert_eq!(cloned.get(ix(2)), None);
        assert_eq!(cloned.get(ix(3)), Some(&3));
    }
}
