// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::support::btree_table::MapBTreeTable;

#[derive(Clone, Debug, Default)]
pub(super) struct IdBTreeMapTables {
    pub(super) key_to_entry: MapBTreeTable,
}

impl IdBTreeMapTables {
    pub(super) fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: crate::test_utils::ValidateCompact,
    ) -> anyhow::Result<()> {
        // Check that all the maps are of the right size.
        use anyhow::Context;

        self.key_to_entry
            .validate(expected_len, compactness)
            .context("key_to_entry failed validation")?;

        Ok(())
    }
}
