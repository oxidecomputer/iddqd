// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    support::hash_table::{MapHash, MapHashTable},
    TriHashMapEntry,
};

#[derive(Clone, Debug, Default)]
pub(super) struct TriHashMapTables {
    pub(super) k1_to_entry: MapHashTable,
    pub(super) k2_to_entry: MapHashTable,
    pub(super) k3_to_entry: MapHashTable,
}

impl TriHashMapTables {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            k1_to_entry: MapHashTable::with_capacity(capacity),
            k2_to_entry: MapHashTable::with_capacity(capacity),
            k3_to_entry: MapHashTable::with_capacity(capacity),
        }
    }

    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: crate::internal::ValidateCompact,
    ) -> anyhow::Result<()> {
        // Check that all the maps are of the right size.

        use anyhow::Context;
        self.k1_to_entry
            .validate(expected_len, compactness)
            .context("k1_to_entry failed validation")?;
        self.k2_to_entry
            .validate(expected_len, compactness)
            .context("k2_to_entry failed validation")?;
        self.k3_to_entry
            .validate(expected_len, compactness)
            .context("k3_to_entry failed validation")?;

        Ok(())
    }

    pub(super) fn make_hashes<T: TriHashMapEntry>(
        &self,
        item: &T,
    ) -> [MapHash; 3] {
        let k1 = item.key1();
        let k2 = item.key2();
        let k3 = item.key3();

        let h1 = self.k1_to_entry.compute_hash(k1);
        let h2 = self.k2_to_entry.compute_hash(k2);
        let h3 = self.k3_to_entry.compute_hash(k3);

        [h1, h2, h3]
    }
}
