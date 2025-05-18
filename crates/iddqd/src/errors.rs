// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Error types for this crate.
//!
//! These types are shared across all map implementations in this crate.

use std::fmt;

/// An error type returned when an entry is inserted that conflicts with
/// existing entries.
#[derive(Debug)]
pub struct DuplicateEntry<T, D = T> {
    new: T,
    duplicates: Vec<D>,
}

impl<T, D> DuplicateEntry<T, D> {
    /// Creates a new `DuplicateEntry` error.
    #[doc(hidden)]
    pub fn __internal_new(new: T, duplicates: Vec<D>) -> Self {
        DuplicateEntry { new, duplicates }
    }

    /// Returns the new entry that was attempted to be inserted.
    #[inline]
    pub fn new_entry(&self) -> &T {
        &self.new
    }

    /// Returns the list of entries that conflict with the new entry.
    #[inline]
    pub fn duplicates(&self) -> &[D] {
        &self.duplicates
    }

    /// Converts self into its constituent parts.
    pub fn into_parts(self) -> (T, Vec<D>) {
        (self.new, self.duplicates)
    }
}

impl<T: Clone> DuplicateEntry<T, &T> {
    /// Converts self to an owned `DuplicateEntry` by cloning the list of
    /// duplicates.
    ///
    /// If `T` is `'static`, the owned form is suitable for conversion to
    /// `Box<dyn std::error::Error>`, `anyhow::Error`, and so on.
    pub fn into_owned(self) -> DuplicateEntry<T> {
        DuplicateEntry {
            new: self.new,
            duplicates: self.duplicates.into_iter().cloned().collect(),
        }
    }
}

impl<T: fmt::Debug, D: fmt::Debug> fmt::Display for DuplicateEntry<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "new entry: {:?} conflicts with existing: {:?}",
            self.new, self.duplicates
        )
    }
}

impl<T: fmt::Debug, D: fmt::Debug> std::error::Error for DuplicateEntry<T, D> {}
