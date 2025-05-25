use crate::{
    TriHashItem,
    internal::{ValidateCompact, ValidationError},
    support::{alloc::Allocator, hash_table::MapHashTable, map_hash::MapHash},
};
use core::hash::BuildHasher;

#[derive(Clone, Debug, Default)]
pub(super) struct TriHashMapTables<S, A: Allocator> {
    pub(super) k1_to_item: MapHashTable<S, A>,
    pub(super) k2_to_item: MapHashTable<S, A>,
    pub(super) k3_to_item: MapHashTable<S, A>,
}

impl<S: Clone + BuildHasher, A: Clone + Allocator> TriHashMapTables<S, A> {
    pub(super) fn with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        alloc: A,
    ) -> Self {
        Self {
            k1_to_item: MapHashTable::with_capacity_and_hasher_in(
                capacity,
                hasher.clone(),
                alloc.clone(),
            ),
            k2_to_item: MapHashTable::with_capacity_and_hasher_in(
                capacity,
                hasher.clone(),
                alloc.clone(),
            ),
            k3_to_item: MapHashTable::with_capacity_and_hasher_in(
                capacity,
                hasher.clone(),
                alloc,
            ),
        }
    }
}

impl<S: Clone + BuildHasher, A: Allocator> TriHashMapTables<S, A> {
    #[cfg(feature = "daft")]
    pub(super) fn hasher(&self) -> &S {
        self.k1_to_item.state()
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

    pub(super) fn make_hashes<T: TriHashItem>(
        &self,
        item: &T,
    ) -> [MapHash<S>; 3] {
        let k1 = item.key1();
        let k2 = item.key2();
        let k3 = item.key3();

        let h1 = self.k1_to_item.compute_hash(k1);
        let h2 = self.k2_to_item.compute_hash(k2);
        let h3 = self.k3_to_item.compute_hash(k3);

        [h1, h2, h3]
    }
}
