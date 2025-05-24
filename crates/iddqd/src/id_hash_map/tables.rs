use crate::{
    IdHashItem,
    internal::{ValidateCompact, ValidationError},
    support::{hash_table::MapHashTable, map_hash::MapHash},
};
use core::hash::BuildHasher;

#[derive(Clone, Debug, Default)]
pub(super) struct IdHashMapTables<S> {
    pub(super) key_to_item: MapHashTable<S>,
}

impl<S: Clone + BuildHasher> IdHashMapTables<S> {
    #[cfg(feature = "daft")]
    pub(crate) fn hasher(&self) -> &S {
        // TODO: store hasher here
        self.key_to_item.state()
    }

    pub(super) fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            key_to_item: MapHashTable::with_capacity_and_hasher(
                capacity, hasher,
            ),
        }
    }

    pub(super) fn validate(
        &self,
        expected_len: usize,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.key_to_item.validate(expected_len, compactness).map_err(
            |error| ValidationError::Table { name: "key_to_table", error },
        )?;

        Ok(())
    }

    pub(super) fn make_hash<T: IdHashItem>(&self, item: &T) -> MapHash<S> {
        let k1 = item.key();
        self.key_to_item.compute_hash(k1)
    }

    pub(super) fn make_key_hash<T: IdHashItem>(
        &self,
        key: &T::Key<'_>,
    ) -> MapHash<S> {
        self.key_to_item.compute_hash(key)
    }
}
