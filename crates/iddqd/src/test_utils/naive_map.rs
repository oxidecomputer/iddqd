// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::TestEntry;
use crate::errors::DuplicateEntry;

/// A naive, inefficient map that acts as an oracle for property-based tests.
///
/// This map is stored as a vector without internal indexes, and performs linear
/// scans.
#[derive(Debug)]
pub(crate) struct NaiveMap {
    entries: Vec<TestEntry>,
    unique_constraint: UniqueConstraint,
}

impl NaiveMap {
    pub(crate) fn new_key1() -> Self {
        Self { entries: Vec::new(), unique_constraint: UniqueConstraint::Key1 }
    }

    // Will use in the future.
    #[expect(unused)]
    pub(crate) fn new_key12() -> Self {
        Self { entries: Vec::new(), unique_constraint: UniqueConstraint::Key12 }
    }

    pub(crate) fn new_key123() -> Self {
        Self {
            entries: Vec::new(),
            unique_constraint: UniqueConstraint::Key123,
        }
    }

    pub(crate) fn insert_unique(
        &mut self,
        entry: TestEntry,
    ) -> Result<(), DuplicateEntry<TestEntry, &TestEntry>> {
        // Cannot store the duplicates directly here because of borrow
        // checker issues. Instead, we store indexes and then map them to
        // entries.
        let dup_indexes = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                self.unique_constraint.matches(&entry, e).then_some(i)
            })
            .collect::<Vec<_>>();

        if dup_indexes.is_empty() {
            self.entries.push(entry);
            Ok(())
        } else {
            Err(DuplicateEntry::new(
                entry,
                dup_indexes.iter().map(|&i| &self.entries[i]).collect(),
            ))
        }
    }

    pub(crate) fn get1(&self, key1: u8) -> Option<&TestEntry> {
        self.entries.iter().find(|e| e.key1 == key1)
    }

    pub(crate) fn get2(&self, key2: char) -> Option<&TestEntry> {
        self.entries.iter().find(|e| e.key2 == key2)
    }

    pub(crate) fn get3(&self, key3: &str) -> Option<&TestEntry> {
        self.entries.iter().find(|e| e.key3 == key3)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &TestEntry> {
        self.entries.iter()
    }
}

/// Which keys to check uniqueness against.
#[derive(Clone, Copy, Debug)]
enum UniqueConstraint {
    Key1,
    Key12,
    Key123,
}

impl UniqueConstraint {
    fn matches(&self, entry: &TestEntry, other: &TestEntry) -> bool {
        match self {
            UniqueConstraint::Key1 => entry.key1 == other.key1,
            UniqueConstraint::Key12 => {
                entry.key1 == other.key1 || entry.key2 == other.key2
            }
            UniqueConstraint::Key123 => {
                entry.key1 == other.key1
                    || entry.key2 == other.key2
                    || entry.key3 == other.key3
            }
        }
    }
}
