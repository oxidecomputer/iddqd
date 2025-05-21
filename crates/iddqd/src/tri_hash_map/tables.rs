use crate::{
    TriHashItem,
    internal::{ValidateCompact, ValidationError},
    support::{hash_table::MapHashTable, map_hash::MapHash},
};

#[derive(Clone, Debug, Default)]
pub(super) struct TriHashMapTables {
    pub(super) k1_to_item: MapHashTable,
    pub(super) k2_to_item: MapHashTable,
    pub(super) k3_to_item: MapHashTable,
}

impl TriHashMapTables {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            k1_to_item: MapHashTable::with_capacity(capacity),
            k2_to_item: MapHashTable::with_capacity(capacity),
            k3_to_item: MapHashTable::with_capacity(capacity),
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
        self.k3_to_item.validate(expected_len, compactness).map_err(
            |error| ValidationError::Table { name: "k3_to_table", error },
        )?;

        Ok(())
    }

    pub(super) fn make_hashes<T: TriHashItem>(&self, item: &T) -> [MapHash; 3] {
        let k1 = item.key1();
        let k2 = item.key2();
        let k3 = item.key3();

        let h1 = self.k1_to_item.compute_hash(k1);
        let h2 = self.k2_to_item.compute_hash(k2);
        let h3 = self.k3_to_item.compute_hash(k3);

        [h1, h2, h3]
    }
}
