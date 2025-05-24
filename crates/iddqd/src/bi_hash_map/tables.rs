use crate::{
    BiHashItem,
    internal::{ValidateCompact, ValidationError},
    support::{hash_table::MapHashTable, map_hash::MapHash},
};
use core::hash::BuildHasher;

#[derive(Clone, Debug, Default)]
pub(super) struct BiHashMapTables<S> {
    pub(super) k1_to_item: MapHashTable<S>,
    pub(super) k2_to_item: MapHashTable<S>,
}

impl<S: Clone + BuildHasher> BiHashMapTables<S> {
    pub(super) fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            k1_to_item: MapHashTable::with_capacity_and_hasher(
                capacity,
                hasher.clone(),
            ),
            k2_to_item: MapHashTable::with_capacity_and_hasher(
                capacity,
                hasher.clone(),
            ),
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
    ) -> [MapHash<S>; 2] {
        let h1 = self.k1_to_item.compute_hash(k1);
        let h2 = self.k2_to_item.compute_hash(k2);

        [h1, h2]
    }
}
