// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::IdOrdItem;
use crate::{
    internal::{ValidateCompact, ValidationError},
    support::{btree_table::MapBTreeTable, map_hash::MapHash},
};
use std::hash::Hash;

#[derive(Clone, Debug, Default)]
pub(super) struct IdOrdMapTables {
    pub(super) key_to_item: MapBTreeTable,
}

impl IdOrdMapTables {
    pub(super) fn new() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.key_to_item.validate(expected_len, compactness).map_err(
            |error| ValidationError::Table { name: "key_to_item", error },
        )?;

        Ok(())
    }

    pub(super) fn make_hash<T: IdOrdItem>(&self, item: &T) -> MapHash
    where
        for<'k> T::Key<'k>: Hash,
    {
        self.key_to_item.compute_hash(item.key())
    }
}
