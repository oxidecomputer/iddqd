//! A newtype identifying items within an `ItemSet`.

use core::fmt;

/// An index identifying an item within an
/// [`ItemSet`](super::item_set::ItemSet).
///
/// We use a `u32` and not a `usize` for these indexes because the increased
/// density leads to meaningful performance improvements on 64-bit targets. This
/// does mean that the maximum number of items is limited to 2^32 - 1 (we
/// reserve `u32::MAX` for the sentinel), but we consider that to be a
/// reasonable tradeoff. The limit is enforced within
/// [`ItemSet::assert_can_grow`]; see [`Self::MAX_VALID`].
///
/// [`ItemSet::assert_can_grow`]: super::item_set::ItemSet::assert_can_grow
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ItemIndex(u32);

impl ItemIndex {
    /// The smallest possible index.
    pub(crate) const ZERO: Self = Self(0);

    /// The largest index that may be assigned to an item.
    ///
    /// One below `u32::MAX`, which is reserved as [`Self::SENTINEL`].
    /// Equivalently, the maximum number of items that may ever be inserted
    /// into a single map (across the map's lifetime, since indexes are never
    /// reused other than through `last_index`) is `u32::MAX`.
    pub(crate) const MAX_VALID: Self = Self(u32::MAX - 1);

    /// Reserved sentinel value marking the root/empty slot. Never assigned to
    /// an item.
    #[cfg_attr(not(feature = "std"), expect(dead_code))]
    pub(crate) const SENTINEL: Self = Self(u32::MAX);

    /// Wraps a raw `u32`.
    #[inline]
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying `u32`.
    #[inline]
    pub(crate) const fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns this index plus one, panicking on overflow.
    ///
    /// Used by [`ItemSet`](super::item_set::ItemSet) to advance
    /// `next_index` after an insert.
    #[inline]
    pub(crate) fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("ItemIndex did not overflow"))
    }

    /// Returns this index minus one, panicking on underflow.
    ///
    /// Used by [`ItemSet`](super::item_set::ItemSet) to roll back
    /// `next_index` when removing the highest-index item.
    #[inline]
    pub(crate) fn prev(self) -> Self {
        Self(self.0.checked_sub(1).expect("ItemIndex did not underflow"))
    }
}

impl fmt::Debug for ItemIndex {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for ItemIndex {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
