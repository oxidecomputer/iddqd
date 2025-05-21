use crate::{
    internal::{ValidateCompact, ValidationError},
    support::{hash_table::MapHashTable, map_hash::MapHash},
    BiHashItem,
};

#[derive(Clone, Debug, Default)]
pub(super) struct BiHashMapTables {
    pub(super) k1_to_item: MapHashTable,
    pub(super) k2_to_item: MapHashTable,
}

impl BiHashMapTables {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            k1_to_item: MapHashTable::with_capacity(capacity),
            k2_to_item: MapHashTable::with_capacity(capacity),
        }
    }

    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        // Check that all the maps are of the right size.
        self.k1_to_item.validate(expected_len, compactness).map_err(
            |error| ValidationError::Table { name: "k1_to_table", error },
        )?;
        self.k2_to_item.validate(expected_len, compactness).map_err(
            |error| ValidationError::Table { name: "k2_to_table", error },
        )?;

        Ok(())
    }

    pub(super) fn make_hashes<T: BiHashItem>(
        &self,
        k1: &T::K1<'_>,
        k2: &T::K2<'_>,
    ) -> [MapHash; 2] {
        let h1 = self.k1_to_item.compute_hash(k1);
        let h2 = self.k2_to_item.compute_hash(k2);

        [h1, h2]
    }
}
