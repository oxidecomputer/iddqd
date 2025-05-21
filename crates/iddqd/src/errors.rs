//! Error types for this crate.
//!
//! These types are shared across all map implementations in this crate.

use alloc::vec::Vec;
use core::fmt;

/// An item conflicts with existing items.
#[derive(Debug)]
pub struct DuplicateItem<T, D = T> {
    new: T,
    duplicates: Vec<D>,
}

impl<T, D> DuplicateItem<T, D> {
    /// Creates a new `DuplicateItem` error.
    #[doc(hidden)]
    pub fn __internal_new(new: T, duplicates: Vec<D>) -> Self {
        DuplicateItem { new, duplicates }
    }

    /// Returns the new item that was attempted to be inserted.
    #[inline]
    pub fn new_item(&self) -> &T {
        &self.new
    }

    /// Returns the list of items that conflict with the new item.
    #[inline]
    pub fn duplicates(&self) -> &[D] {
        &self.duplicates
    }

    /// Converts self into its constituent parts.
    pub fn into_parts(self) -> (T, Vec<D>) {
        (self.new, self.duplicates)
    }
}

impl<T: Clone> DuplicateItem<T, &T> {
    /// Converts self to an owned `DuplicateItem` by cloning the list of
    /// duplicates.
    ///
    /// If `T` is `'static`, the owned form is suitable for conversion to
    /// `Box<dyn std::error::Error>`, `anyhow::Error`, and so on.
    pub fn into_owned(self) -> DuplicateItem<T> {
        DuplicateItem {
            new: self.new,
            duplicates: self.duplicates.into_iter().cloned().collect(),
        }
    }
}

impl<T: fmt::Debug, D: fmt::Debug> fmt::Display for DuplicateItem<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "new item: {:?} conflicts with existing: {:?}",
            self.new, self.duplicates
        )
    }
}

impl<T: fmt::Debug, D: fmt::Debug> core::error::Error for DuplicateItem<T, D> {}
