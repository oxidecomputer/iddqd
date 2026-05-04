//! A newtype identifying items within an `ItemSet`.

use core::fmt;

/// An index identifying an item within an
/// [`ItemSet`](super::item_set::ItemSet).
///
/// We use a `u32` and not a `usize` for these indexes because the increased
/// density leads to meaningful performance improvements on 64-bit targets. This
/// does mean that the maximum number of concurrently live items is limited to
/// `u32::MAX - 1` slots (the -1 is because `u32::MAX` is reserved for
/// [`Self::SENTINEL`]). This limit is enforced within
/// [`ItemSet::assert_can_grow`].
///
/// [`ItemSet::assert_can_grow`]: super::item_set::ItemSet::assert_can_grow
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ItemIndex(u32);

impl ItemIndex {
    /// The largest index that may be assigned to an item.
    ///
    /// One below `u32::MAX`, which is reserved as [`Self::SENTINEL`].
    pub(crate) const MAX_VALID: Self = Self(u32::MAX - 1);

    /// Reserved sentinel value marking the root/empty slot. Never assigned to
    /// an item.
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
