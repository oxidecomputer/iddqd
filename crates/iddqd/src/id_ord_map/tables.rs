use super::IdOrdItem;
use crate::{
    internal::{ValidateCompact, ValidationError},
    support::{btree_table::MapBTreeTable, map_hash::MapHash},
};
use core::hash::Hash;

#[derive(Clone, Debug, Default)]
pub(super) struct IdOrdMapTables {
    pub(super) key_to_item: MapBTreeTable,
}

impl IdOrdMapTables {
    pub(super) const fn new() -> Self {
        Self { key_to_item: MapBTreeTable::new() }
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

    pub(super) fn make_hash<'a, T>(
        &self,
        item: &'a T,
    ) -> MapHash<foldhash::fast::FixedState>
    where
        T::Key<'a>: Hash,
        T: 'a + IdOrdItem,
    {
        self.key_to_item.compute_hash(item.key())
    }
}
