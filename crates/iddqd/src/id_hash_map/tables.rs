// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    support::hash_table::{MapHash, MapHashTable},
    IdHashItem,
};

#[derive(Clone, Debug, Default)]
pub(super) struct IdHashMapTables {
    pub(super) key_to_item: MapHashTable,
}

impl IdHashMapTables {
    pub(super) fn new() -> Self {
        Self { key_to_item: MapHashTable::default() }
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self { key_to_item: MapHashTable::with_capacity(capacity) }
    }

    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: crate::internal::ValidateCompact,
    ) -> anyhow::Result<()> {
        // Check that all the maps are of the right size.

        use anyhow::Context;
        self.key_to_item
            .validate(expected_len, compactness)
            .context("k1_to_item failed validation")?;

        Ok(())
    }

    pub(super) fn make_hash<T: IdHashItem>(&self, item: &T) -> MapHash {
        let k1 = item.key();
        self.key_to_item.compute_hash(k1)
    }

    pub(super) fn make_key_hash<T: IdHashItem>(
        &self,
        key: &T::Key<'_>,
    ) -> MapHash {
        self.key_to_item.compute_hash(key)
    }
}
