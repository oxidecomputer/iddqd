//! Crate-internal support for classifying multi-key entry lookups.
//!
//! This module is intentionally independent of the map implementations. It only
//! understands fixed arrays of optional item indexes, preserving enough state for
//! entry APIs to reason about vacant, unique, and non-unique lookup results.

use crate::support::ItemIndex;

/// Classification of a multi-key entry lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryLookup<const N: usize> {
    /// No key matched an existing item.
    Vacant,
    /// Every key matched the same existing item.
    Unique(ItemIndex),
    /// At least one key matched, but the lookup was not unique.
    NonUnique(NonUniqueIndexes<N>),
}

/// Per-key lookup indexes for an entry operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EntryIndexes<const N: usize> {
    indexes: [Option<ItemIndex>; N],
}

/// Non-unique per-key lookup indexes.
///
/// Invariant: at least one index is `Some`, and the indexes are not all the
/// same `Some` value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NonUniqueIndexes<const N: usize> {
    indexes: [Option<ItemIndex>; N],
}

/// Distinct indexes referenced by a non-vacant lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DistinctIndexes<const N: usize> {
    indexes: [Option<ItemIndex>; N],
    len: usize,
    key_to_slot: [Option<usize>; N],
}

impl<const N: usize> EntryIndexes<N> {
    #[inline]
    pub(crate) const fn new(indexes: [Option<ItemIndex>; N]) -> Self {
        Self { indexes }
    }

    #[inline]
    #[expect(
        dead_code,
        reason = "reserved for upcoming TriHashMap occupied entry replacement validation"
    )]
    pub(crate) const fn indexes(&self) -> &[Option<ItemIndex>; N] {
        &self.indexes
    }

    #[inline]
    pub(crate) fn classify(self) -> EntryLookup<N> {
        let mut first = None;
        let mut saw_none = false;
        let mut all_some_same = true;

        for index in self.indexes {
            match (first, index) {
                (None, Some(index)) => first = Some(index),
                (Some(first_index), Some(index)) if first_index != index => {
                    all_some_same = false;
                }
                (_, None) => saw_none = true,
                _ => {}
            }
        }

        match (first, saw_none, all_some_same) {
            (None, _, _) => EntryLookup::Vacant,
            (Some(index), false, true) => EntryLookup::Unique(index),
            (Some(_), _, _) => EntryLookup::NonUnique(NonUniqueIndexes {
                indexes: self.indexes,
            }),
        }
    }
}

impl<const N: usize> NonUniqueIndexes<N> {
    #[inline]
    pub(crate) const fn indexes(&self) -> &[Option<ItemIndex>; N] {
        &self.indexes
    }

    #[inline]
    pub(crate) fn distinct(self) -> DistinctIndexes<N> {
        DistinctIndexes::from_indexes(self.indexes)
    }
}

impl<const N: usize> DistinctIndexes<N> {
    fn from_indexes(source: [Option<ItemIndex>; N]) -> Self {
        let mut indexes = [None; N];
        let mut key_to_slot = [None; N];
        let mut len = 0;

        for (key, source_index) in source.into_iter().enumerate() {
            if let Some(source_index) = source_index {
                let mut slot = None;

                // Distinct indexes are stored densely in first-key-hit order.
                // Only the initialized prefix `..len` is inspected here.
                for (candidate_slot, candidate) in
                    indexes[..len].iter().enumerate()
                {
                    if *candidate == Some(source_index) {
                        slot = Some(candidate_slot);
                        break;
                    }
                }

                let slot = match slot {
                    Some(slot) => slot,
                    None => {
                        let slot = len;
                        indexes[slot] = Some(source_index);
                        len += 1;
                        slot
                    }
                };
                key_to_slot[key] = Some(slot);
            }
        }

        Self { indexes, len, key_to_slot }
    }

    #[inline]
    pub(crate) const fn indexes(&self) -> &[Option<ItemIndex>; N] {
        &self.indexes
    }

    #[inline]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub(crate) const fn key_to_slot(&self) -> &[Option<usize>; N] {
        &self.key_to_slot
    }
}

#[cfg(test)]
mod tests {
    use super::{EntryIndexes, EntryLookup};
    use crate::support::ItemIndex;

    fn ix(value: u32) -> ItemIndex {
        ItemIndex::new(value)
    }

    fn classify<const N: usize>(
        indexes: [Option<ItemIndex>; N],
    ) -> EntryLookup<N> {
        EntryIndexes::new(indexes).classify()
    }

    fn non_unique_distinct<const N: usize>(
        indexes: [Option<ItemIndex>; N],
    ) -> (usize, [Option<ItemIndex>; N], [Option<usize>; N]) {
        let EntryLookup::NonUnique(indexes) = classify(indexes) else {
            panic!("expected non-unique indexes")
        };
        let distinct = indexes.distinct();
        (distinct.len(), *distinct.indexes(), *distinct.key_to_slot())
    }

    #[test]
    fn arity_2_vacant_classification() {
        assert_eq!(classify([None, None]), EntryLookup::Vacant);
    }

    #[test]
    fn arity_2_unique_classification() {
        assert_eq!(
            classify([Some(ix(1)), Some(ix(1))]),
            EntryLookup::Unique(ix(1))
        );
    }

    #[test]
    fn arity_2_partial_classification() {
        assert!(matches!(
            classify([Some(ix(1)), None]),
            EntryLookup::NonUnique(_)
        ));
        assert!(matches!(
            classify([None, Some(ix(1))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn arity_2_mixed_classification() {
        assert!(matches!(
            classify([Some(ix(1)), Some(ix(2))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn arity_3_vacant_classification() {
        assert_eq!(classify([None, None, None]), EntryLookup::Vacant);
    }

    #[test]
    fn arity_3_unique_classification() {
        assert_eq!(
            classify([Some(ix(1)), Some(ix(1)), Some(ix(1))]),
            EntryLookup::Unique(ix(1))
        );
    }

    #[test]
    fn arity_3_partial_duplicate_classification() {
        assert!(matches!(
            classify([Some(ix(1)), Some(ix(1)), None]),
            EntryLookup::NonUnique(_)
        ));
        assert!(matches!(
            classify([None, Some(ix(1)), Some(ix(1))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn arity_3_separated_duplicate_classification() {
        assert!(matches!(
            classify([Some(ix(1)), None, Some(ix(1))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn arity_3_mixed_duplicate_classification() {
        assert!(matches!(
            classify([Some(ix(1)), Some(ix(1)), Some(ix(2))]),
            EntryLookup::NonUnique(_)
        ));
        assert!(matches!(
            classify([Some(ix(1)), Some(ix(2)), Some(ix(1))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn arity_3_all_distinct_classification() {
        assert!(matches!(
            classify([Some(ix(1)), Some(ix(2)), Some(ix(3))]),
            EntryLookup::NonUnique(_)
        ));
    }

    #[test]
    fn deterministic_first_key_hit_distinct_ordering() {
        assert_eq!(
            non_unique_distinct([Some(ix(1)), Some(ix(1)), Some(ix(2))]),
            (2, [Some(ix(1)), Some(ix(2)), None], [Some(0), Some(0), Some(1)])
        );
        assert_eq!(
            non_unique_distinct([Some(ix(1)), Some(ix(2)), Some(ix(1))]),
            (2, [Some(ix(1)), Some(ix(2)), None], [Some(0), Some(1), Some(0)])
        );
        assert_eq!(
            non_unique_distinct([None, Some(ix(2)), Some(ix(1))]),
            (2, [Some(ix(2)), Some(ix(1)), None], [None, Some(0), Some(1)])
        );
    }

    #[test]
    fn key_to_slot_mapping_for_repeated_indexes() {
        assert_eq!(
            non_unique_distinct([Some(ix(1)), None, Some(ix(1))]),
            (1, [Some(ix(1)), None, None], [Some(0), None, Some(0)])
        );
    }

    #[test]
    fn no_duplicate_distinct_indexes() {
        assert_eq!(
            non_unique_distinct([Some(ix(1)), Some(ix(2)), Some(ix(3))]),
            (
                3,
                [Some(ix(1)), Some(ix(2)), Some(ix(3))],
                [Some(0), Some(1), Some(2)]
            )
        );
    }
}
