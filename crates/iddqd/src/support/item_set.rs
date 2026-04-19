use super::{alloc::AllocWrapper, free_list::FreeList};
use crate::{
    errors::TryReserveError,
    internal::{ValidateCompact, ValidationError},
    support::alloc::{Allocator, Global, global_alloc},
};
use allocator_api2::vec::Vec;
use core::{
    fmt,
    iter::FusedIterator,
    ops::{Index, IndexMut},
};

/// A remap from old (pre-compaction) to new (post-compaction) indexes.
///
/// Produced by [`ItemSet::shrink_to_fit`] and [`ItemSet::shrink_to`],
/// consumed by the outer tables (hash / btree index tables) so they
/// can rewrite their stored indexes to point at the compacted `items`
/// buffer.
///
/// `holes` is the sorted list of pre-compaction indexes that were
/// compacted away. Under the invariants of [`ItemSet`], the holes are
/// exactly the vacant slots that existed at the time of the shrink —
/// equivalently, the pre-shrink free list in sorted order.
#[derive(Clone, Debug)]
pub(crate) struct IndexRemap {
    holes: alloc::vec::Vec<usize>,
}

impl IndexRemap {
    /// Returns `true` if no indexes were compacted away (i.e. the
    /// shrink was a capacity-only operation and the outer tables do
    /// not need rewriting).
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.holes.is_empty()
    }

    /// Translates an old (pre-compaction) index into its new
    /// (post-compaction) position.
    ///
    /// `old` must be an index that is still live after compaction —
    /// callers walk the outer tables, whose entries always point at
    /// live items, so they only ever pass live indexes here.
    ///
    /// Runs in `O(log k)` where `k = self.holes.len()`.
    #[inline]
    pub(crate) fn remap(&self, old: usize) -> usize {
        // `partition_point` returns the count of holes strictly less
        // than `old` — each of those holes shifted `old` down by one.
        let shift = self.holes.partition_point(|&h| h < old);
        debug_assert!(
            self.holes.binary_search(&old).is_err(),
            "IndexRemap::remap called on a compacted-away index {old}"
        );
        old - shift
    }
}

/// A dense, index-keyed container for items.
///
/// # Design
///
/// Items live in `items: Vec<Option<T>>`, indexed directly without any
/// hashing. Slots freed by [`remove`](Self::remove) land on `free_list`
/// and are reused by the next
/// [`insert_at_next_index`](Self::insert_at_next_index), so a churn
/// workload stabilizes at the high-water mark of simultaneously-live
/// items rather than the cumulative insertion count.
///
/// Using `Option<T>` keeps the hot-path `get` a single cache-line access:
/// for any `T` whose layout has a niche (e.g. anything containing a
/// `Box`, `Vec`, `String`, `&T`, `NonZero*`, and so on), `Option<T>` has
/// the same size as `T` and "is it present?" compiles to a null-pointer
/// test.
///
/// The free list lives behind a single nullable pointer
/// ([`FreeList`]) which is lazily allocated on first use. That keeps
/// `ItemSet`'s own footprint to a single word beyond `items` and
/// avoids any heap traffic for build-and-read or grow-only maps.
///
/// # Invariants
///
/// 1. For every `i < items.len()`: `items[i]` is `Some` iff `i` is not
///    currently in the free list.
/// 2. The free list contains no duplicates and no out-of-bounds indexes.
///
/// `items` is not eagerly compacted: a trailing remove leaves a `None`
/// slot (and the matching free-list entry) in place. Trailing vacancies
/// are only reclaimed by [`shrink_to_fit`](Self::shrink_to_fit) or
/// [`shrink_to`](Self::shrink_to). This keeps [`remove`](Self::remove)
/// uniform (a single `slot.take` + `free_list.push`) regardless of
/// position; the next [`insert_at_next_index`](Self::insert_at_next_index)
/// reuses the vacated slot via the free list at no memory cost.
///
/// The live item count is derived — not stored — as
/// `items.len() - free_list.len()`. Under invariants 1–2 that equals
/// the number of `Some` entries in `items`.
pub(crate) struct ItemSet<T, A: Allocator> {
    items: Vec<Option<T>, AllocWrapper<A>>,
    free_list: FreeList<A>,
}

impl<T, A: Allocator> Drop for ItemSet<T, A> {
    fn drop(&mut self) {
        // Deallocate the free list first, while we can still reach the
        // allocator via `items`. `items` gets dropped automatically
        // afterward.
        self.free_list.deallocate(self.items.allocator());
    }
}

impl<T: Clone, A: Clone + Allocator> Clone for ItemSet<T, A> {
    fn clone(&self) -> Self {
        let mut new =
            Self { items: self.items.clone(), free_list: FreeList::new() };
        for &idx in self.free_list.as_slice() {
            // SAFETY: `new.free_list` is fresh, so this is the first
            // push — the same-allocator contract is trivially
            // satisfied on this call, and every subsequent site reuses
            // `new.items.allocator()`.
            unsafe {
                new.free_list.push(idx, new.items.allocator());
            }
        }
        new
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for ItemSet<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemSet")
            .field("len", &self.len())
            .field("slots", &self.items.len())
            .field("free_list", &self.free_list.as_slice())
            .finish()
    }
}

impl<T> ItemSet<T, Global> {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            items: Vec::new_in(AllocWrapper(global_alloc())),
            free_list: FreeList::new(),
        }
    }
}

impl<T, A: Allocator> ItemSet<T, A> {
    #[inline]
    pub(crate) const fn new_in(alloc: A) -> Self {
        Self {
            items: Vec::new_in(AllocWrapper(alloc)),
            free_list: FreeList::new(),
        }
    }

    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: Vec::with_capacity_in(capacity, AllocWrapper(alloc)),
            free_list: FreeList::new(),
        }
    }

    pub(crate) fn allocator(&self) -> &A {
        &self.items.allocator().0
    }

    /// Returns a raw pointer to the backing slot buffer.
    ///
    /// Intended for iterator types that need to hand out disjoint
    /// `&mut T` across iterations without reborrowing `&mut ItemSet`
    /// each time (which under Tree Borrows would invalidate previously
    /// yielded references).
    #[inline]
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut Option<T> {
        self.items.as_mut_ptr()
    }

    pub(crate) fn validate(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        let some_count = self.items.iter().filter(|s| s.is_some()).count();
        let free = self.free_list.as_slice();
        if self.items.len() - some_count != free.len() {
            return Err(ValidationError::General(format!(
                "ItemSet free_list size ({}) inconsistent with vacant \
                 slot count ({})",
                free.len(),
                self.items.len() - some_count,
            )));
        }
        for &idx in free {
            if idx >= self.items.len() {
                return Err(ValidationError::General(format!(
                    "ItemSet free_list has out-of-range index {idx}"
                )));
            }
            if self.items[idx].is_some() {
                return Err(ValidationError::General(format!(
                    "ItemSet free_list has occupied index {idx}"
                )));
            }
        }
        let mut sorted: alloc::vec::Vec<usize> = free.to_vec();
        sorted.sort_unstable();
        for pair in sorted.windows(2) {
            if pair[0] == pair[1] {
                return Err(ValidationError::General(format!(
                    "ItemSet free_list has duplicate index {}",
                    pair[0],
                )));
            }
        }
        match compactness {
            ValidateCompact::Compact => {
                if !free.is_empty() {
                    return Err(ValidationError::General(format!(
                        "ItemSet is not compact: free_list has {} entries",
                        free.len(),
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
        // `items.len() == free_list.len()` iff every slot is vacant,
        // which (under invariant 1) means there are no live items.
        self.items.len() == self.free_list.len()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len() - self.free_list.len()
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
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index).and_then(Option::as_ref)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index).and_then(Option::as_mut)
    }

    /// Returns mutable references to up to `N` distinct indexes.
    ///
    /// Returns `None` for any index that is out of bounds, vacant, or
    /// that duplicates an earlier index in the array.
    pub(crate) fn get_disjoint_mut<const N: usize>(
        &mut self,
        indexes: [&usize; N],
    ) -> [Option<&mut T>; N] {
        let len = self.items.len();
        let mut valid = [false; N];
        for i in 0..N {
            let idx = *indexes[i];
            if idx >= len {
                continue;
            }
            // SAFETY: idx < len, so `items[idx]` is in bounds.
            if unsafe { self.items.get_unchecked(idx) }.is_none() {
                continue;
            }
            let mut dup = false;
            for j in 0..i {
                if valid[j] && *indexes[j] == idx {
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
                let idx = *indexes[i];
                // SAFETY: we verified idx is in bounds, the slot is
                // `Some`, and no earlier valid entry shares this index.
                // Therefore the `&mut` references are disjoint.
                unsafe { (*base.add(idx)).as_mut() }
            } else {
                None
            }
        })
    }

    /// Returns the index that [`insert_at_next_index`] will use on its
    /// next call.
    ///
    /// [`insert_at_next_index`]: Self::insert_at_next_index
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    #[inline]
    pub(crate) fn next_index(&self) -> usize {
        // LIFO reuse: the most recently freed slot is likeliest to be
        // cache-hot.
        match self.free_list.last() {
            Some(idx) => idx,
            None => self.items.len(),
        }
    }

    #[inline]
    pub(crate) fn insert_at_next_index(&mut self, value: T) -> usize {
        if let Some(idx) = self.free_list.pop() {
            debug_assert!(self.items[idx].is_none());
            self.items[idx] = Some(value);
            idx
        } else {
            let idx = self.items.len();
            self.items.push(Some(value));
            idx
        }
    }

    /// Removes the item at `index`, if any.
    ///
    /// Pushes `index` onto the free list for reuse by the next
    /// [`insert_at_next_index`](Self::insert_at_next_index). The push
    /// may allocate if the free list hasn't been pre-sized; allocator
    /// failure aborts via the global OOM handler, matching the standard
    /// collections' infallible-allocation convention. Callers that need
    /// a no-OOM guarantee should pre-size up front via
    /// [`try_reserve`](Self::try_reserve).
    ///
    /// `items` is not truncated here, even for a trailing remove — the
    /// vacated slot stays in place until reused by the next insert or
    /// reclaimed by [`shrink_to_fit`](Self::shrink_to_fit).
    #[inline]
    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        let slot = self.items.get_mut(index)?;
        let value = slot.take()?;
        // SAFETY: we're handing the same allocator that owns `items` to
        // the free list, matching every other site that uses it.
        unsafe {
            self.free_list.push(index, self.items.allocator());
        }
        Some(value)
    }

    /// Consumes this set into an owned, invariant-free
    /// [`ConsumingItemSet`].
    ///
    /// Deallocates the free list (the consuming view has no use for it)
    /// and hands over ownership of the items buffer. Used by
    /// [`IntoValues`] and by `IdOrdMap`'s owning iterator, both of which
    /// drain every live slot at most once and don't care about
    /// reconstructing the set.
    pub(crate) fn into_consuming(self) -> ConsumingItemSet<T, A> {
        // `ManuallyDrop` suppresses the `ItemSet` drop glue so we can
        // disassemble the set field-by-field.
        let mut set = core::mem::ManuallyDrop::new(self);

        // Deallocate the free list while `items` (and therefore its
        // allocator) is still live. We resplit the borrows to satisfy
        // the borrow checker: `free_list` wants `&mut`, `items` wants
        // `&`.
        {
            let set = &mut *set;
            set.free_list.deallocate(set.items.allocator());
        }

        // SAFETY: we own `set` by value and the `ManuallyDrop` wrapper prevents
        // any automatic drop of its fields, so moving `items` out by
        // `ptr::read` is sound. `set.items` is no longer accessed after this
        // point.
        let items = unsafe { core::ptr::read(&set.items) };
        ConsumingItemSet { items }
    }

    /// Clears the item set, removing all items.
    ///
    /// Preserves the capacity of both the items buffer and the free
    /// list, matching the behavior of [`Vec::clear`]. A caller that
    /// pre-sized via [`try_reserve`](Self::try_reserve) retains its
    /// no-OOM guarantee across a `clear` and subsequent reuse.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.free_list.clear();
    }

    // This method assumes that value has the same ID. It also asserts
    // that `index` is valid (and panics if it isn't).
    #[inline]
    pub(crate) fn replace(&mut self, index: usize, value: T) -> T {
        match self.items.get_mut(index) {
            Some(slot @ Some(_)) => {
                slot.replace(value).expect("slot was just checked to be Some")
            }
            _ => panic!("ItemSet index not found: {index}"),
        }
    }

    #[inline]
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
    }

    #[inline]
    pub(crate) fn shrink_to_fit(&mut self) -> IndexRemap {
        let remap = self.compact();
        self.items.shrink_to_fit();
        remap
    }

    #[inline]
    pub(crate) fn shrink_to(&mut self, min_capacity: usize) -> IndexRemap {
        let remap = self.compact();
        self.items.shrink_to(min_capacity);
        remap
    }

    /// Moves every live slot down to fill `None` holes, truncates
    /// `items` to its new length, and clears the free list.
    ///
    /// Returns an [`IndexRemap`] whose `holes` are the sorted list of
    /// pre-compaction indexes whose items were shifted away. The remap
    /// is empty iff no compaction happened (no holes existed).
    ///
    /// Items are moved via [`Vec::swap`], so no `T::drop` runs here —
    /// the only `Option::<T>::drop` calls are on the trailing `None`s
    /// popped by `truncate`, and dropping `None` is a no-op. This
    /// preserves the panic-safety invariant captured by
    /// [`shrink_to_fit_does_not_drop_in_place`](tests::shrink_to_fit_does_not_drop_in_place).
    fn compact(&mut self) -> IndexRemap {
        // Snapshot the free list as a sorted `Vec<usize>` — these are
        // the indexes that are about to be filled by the compaction
        // below. We allocate unconditionally here rather than borrow
        // from `self.free_list` because the free list needs to be
        // cleared before we return, and the caller needs the sorted
        // holes to outlive `self.free_list`'s state.
        let mut holes: alloc::vec::Vec<usize> =
            self.free_list.as_slice().to_vec();
        holes.sort_unstable();

        // Two-pointer compaction: scan `items` forward; every `Some`
        // slot gets swapped into the next write position. A `Some`
        // that's already in the right place costs a single `is_some`
        // check.
        let mut write = 0;
        for read in 0..self.items.len() {
            if self.items[read].is_some() {
                if write != read {
                    self.items.swap(write, read);
                }
                write += 1;
            }
        }
        self.items.truncate(write);
        self.free_list.clear();

        IndexRemap { holes }
    }

    /// Tries to reserve capacity for at least `additional` more items.
    ///
    /// Reserves room in both the items buffer and the free list.
    ///
    /// After this call returns `Ok(())`, the next `additional` calls to
    /// [`insert_at_next_index`](Self::insert_at_next_index) and any
    /// interleaved [`remove`](Self::remove) calls are OOM-free. The
    /// guarantee is reset by any of:
    ///
    /// * more than `additional` insertions past this point
    ///   (the items buffer grows through the infallible allocation
    ///   path);
    /// * [`shrink_to_fit`](Self::shrink_to_fit) /
    ///   [`shrink_to`](Self::shrink_to_fit), which may release capacity
    ///   that the reservation was counting on.
    ///
    /// [`clear`](Self::clear) preserves the reservation.
    #[inline]
    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), TryReserveError> {
        self.items
            .try_reserve(additional)
            .map_err(TryReserveError::from_allocator_api2)?;
        // Target free-list capacity: enough to hold the maximum possible
        // vacancies after the caller inserts `additional` items. An
        // upper bound is `self.items.len() + additional`.
        let target = self.items.len().saturating_add(additional);
        if target > 0 {
            // SAFETY: `self.items.allocator()` is the allocator used for
            // every prior free-list mutation, matching the contract;
            // `target > 0` satisfies `try_reserve_total`'s nonzero
            // precondition.
            unsafe {
                self.free_list
                    .try_reserve_total(target, self.items.allocator())?;
            }
        }
        Ok(())
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

impl<T, A: Allocator> Index<usize> for ItemSet<T, A> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}

impl<T, A: Allocator> IndexMut<usize> for ItemSet<T, A> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index)
            .unwrap_or_else(|| panic!("ItemSet index not found: {index}"))
    }
}

// --- Iterators ----------------------------------------------------------

/// An iterator over `(index, &item)` pairs in an [`ItemSet`].
pub(crate) struct Iter<'a, T> {
    inner: core::iter::Enumerate<core::slice::Iter<'a, Option<T>>>,
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
        let empty: &[Option<T>] = &[];
        Self { inner: empty.iter().enumerate(), remaining: 0 }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (usize, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.inner.by_ref() {
            if let Some(v) = slot {
                self.remaining -= 1;
                return Some((i, v));
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
    inner: core::iter::Enumerate<core::slice::IterMut<'a, Option<T>>>,
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
    type Item = (usize, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for (i, slot) in self.inner.by_ref() {
            if let Some(v) = slot {
                self.remaining -= 1;
                return Some((i, v));
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
    inner: core::slice::Iter<'a, Option<T>>,
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
        let empty: &[Option<T>] = &[];
        Self { inner: empty.iter(), remaining: 0 }
    }
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.inner.by_ref().flatten().next()?;
        self.remaining -= 1;
        Some(v)
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
    inner: core::slice::IterMut<'a, Option<T>>,
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
        let v = self.inner.by_ref().flatten().next()?;
        self.remaining -= 1;
        Some(v)
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
    inner: allocator_api2::vec::IntoIter<Option<T>, AllocWrapper<A>>,
    remaining: usize,
}

impl<T, A: Allocator> IntoValues<T, A> {
    fn new(set: ItemSet<T, A>) -> Self {
        // Compute `remaining` before consuming the set: `ItemSet::len()` is
        // derived from `items.len() - free_list.len()`, and `into_consuming`
        // deallocates the free list.
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
        let v = self.inner.by_ref().flatten().next()?;
        self.remaining -= 1;
        Some(v)
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

/// An [`ItemSet`] consumed into an owned, by-index take-only view.
///
/// Produced by [`ItemSet::into_consuming`]. The free list is gone and
/// the invariants of `ItemSet` no longer apply: indexes are taken one at
/// a time via [`take`](Self::take) and the type makes no attempt to
/// reuse vacated slots or maintain a live-count.
///
/// Existing `Some` slots that are never taken are dropped by the underlying
/// `Vec` when `ConsumingItemSet` itself is dropped, so partial consumption
/// does not cause a memory leak.
pub(crate) struct ConsumingItemSet<T, A: Allocator> {
    items: Vec<Option<T>, AllocWrapper<A>>,
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for ConsumingItemSet<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsumingItemSet")
            .field("slots", &self.items.len())
            .finish()
    }
}

impl<T, A: Allocator> ConsumingItemSet<T, A> {
    /// Takes the item at `index`, leaving a `None` slot behind.
    ///
    /// Returns `None` if `index` is out of bounds or the slot has already been
    /// taken. O(1) regardless of position.
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    #[inline]
    pub(crate) fn take(&mut self, index: usize) -> Option<T> {
        self.items.get_mut(index)?.take()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::internal::ValidateCompact;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    /// Drops `T` via a user-supplied closure; if the closure panics,
    /// the panic propagates out of `drop`.
    struct PanickyDrop {
        /// Bumped on every drop call so tests can count how many drops
        /// ran before a panicking one aborted the sequence.
        drop_count: Rc<Cell<usize>>,
        /// When `true`, `drop` panics.
        panic_on_drop: bool,
    }

    impl Drop for PanickyDrop {
        fn drop(&mut self) {
            self.drop_count.set(self.drop_count.get() + 1);
            if self.panic_on_drop {
                panic!("PanickyDrop");
            }
        }
    }

    /// Checks that [`ItemSet::shrink_to_fit`] never drops `T` in place.
    /// Only trailing `None` slots are popped, and `Option::<T>::drop` on
    /// `None` is a no-op, so a panicking `T::drop` cannot unwind through
    /// the shrink path.
    ///
    /// A regression here would leave the free list referencing indexes
    /// past a truncated `items` buffer (invariant 2 violated) or half-
    /// dropped items mid-compaction.
    #[test]
    fn shrink_to_fit_does_not_drop_in_place() {
        let drop_count = Rc::new(Cell::new(0));
        let mk = |panic_on_drop| PanickyDrop {
            drop_count: drop_count.clone(),
            panic_on_drop,
        };

        let mut set = ItemSet::<PanickyDrop, Global>::new();
        // items = [Some(0), Some(1), Some(2), Some(3), Some(4_panicky)]
        for i in 0..5 {
            set.insert_at_next_index(mk(i == 4));
        }
        // Capture the panicky item by removing it first (so its drop
        // never fires inside any ItemSet method). Then vacate the other
        // three non-trailing slots.
        let panicky = set.remove(4).expect("slot was occupied");
        for idx in [3, 2, 1] {
            drop(set.remove(idx).expect("slot was occupied"));
        }
        assert_eq!(drop_count.get(), 3, "three non-panicky items dropped");
        // items = [Some(0), None, None, None, None]
        // free_list = [4, 3, 2, 1]

        // shrink_to_fit pops the four trailing `None` slots and trims
        // the free list. No `T::drop` runs here.
        set.shrink_to_fit();
        assert_eq!(
            drop_count.get(),
            3,
            "shrink_to_fit pops only None slots; no T::drop runs"
        );
        set.validate(ValidateCompact::NonCompact)
            .expect("ItemSet invariants hold after shrink_to_fit");
        assert_eq!(set.len(), 1);

        // Drop the captured panicky item outside any ItemSet method.
        // The panic is caught at this site; the set's state is
        // untouched by that drop.
        let caught = catch_unwind(AssertUnwindSafe(move || drop(panicky)));
        assert!(caught.is_err(), "PanickyDrop panics on drop");
        set.validate(ValidateCompact::NonCompact).expect(
            "ItemSet invariants still hold after the captured-value drop panic",
        );
    }

    /// Checks that [`ItemSet::shrink_to_fit`] compacts away *middle*
    /// holes (not only trailing ones) and returns an [`IndexRemap`]
    /// that maps every surviving index to its new position.
    ///
    /// Before the `IndexRemap` rework, shrink only popped trailing
    /// `None`s, so a map with gaps in the middle retained the dead
    /// slots forever. A regression here would either leave holes in
    /// the compacted `items` buffer, return an incorrect remap, or
    /// drop `T` in place (breaking the panic-safety invariant in
    /// [`shrink_to_fit_does_not_drop_in_place`]).
    #[test]
    fn shrink_to_fit_compacts_middle_holes() {
        let drop_count = Rc::new(Cell::new(0));
        let mk = || PanickyDrop {
            drop_count: drop_count.clone(),
            panic_on_drop: false,
        };

        let mut set = ItemSet::<PanickyDrop, Global>::new();
        // items = [Some(0), Some(1), Some(2), Some(3), Some(4)]
        let indexes: Vec<_, _> =
            (0..5).map(|_| set.insert_at_next_index(mk())).collect();
        assert_eq!(indexes.as_slice(), &[0, 1, 2, 3, 4]);

        // Drop-count check baseline: none have dropped yet.
        assert_eq!(drop_count.get(), 0);

        // Vacate two middle slots. After this:
        //   items = [Some(0), None, Some(2), None, Some(4)]
        //   free_list = [1, 3]   (order is LIFO)
        drop(set.remove(1).expect("slot was occupied"));
        drop(set.remove(3).expect("slot was occupied"));
        assert_eq!(drop_count.get(), 2, "removed items dropped at remove time");

        // Compact.
        let drop_count_before = drop_count.get();
        let remap = set.shrink_to_fit();
        assert_eq!(
            drop_count.get(),
            drop_count_before,
            "shrink_to_fit moves items; no T::drop runs during compaction"
        );

        // Now: items = [Some(<was 0>), Some(<was 2>), Some(<was 4>)],
        // free_list is empty, and the remap records holes [1, 3].
        assert_eq!(set.len(), 3);
        set.validate(ValidateCompact::Compact)
            .expect("ItemSet should be fully compact after shrink_to_fit");

        assert!(!remap.is_empty());
        assert_eq!(remap.remap(0), 0);
        assert_eq!(remap.remap(2), 1);
        assert_eq!(remap.remap(4), 2);
    }

    /// Shrinking a set with no holes is a no-op as far as the remap is
    /// concerned: `IndexRemap::is_empty()` is `true` and outer callers
    /// skip the table rewrite.
    #[test]
    fn shrink_to_fit_without_holes_returns_empty_remap() {
        let mut set = ItemSet::<u32, Global>::new();
        for i in 0..4 {
            set.insert_at_next_index(i);
        }
        let remap = set.shrink_to_fit();
        assert!(remap.is_empty());
        set.validate(ValidateCompact::Compact)
            .expect("a hole-free set is trivially compact after shrink_to_fit");
    }
}
