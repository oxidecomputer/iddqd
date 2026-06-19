use crate::support::{
    ItemIndex,
    entry::{
        EntryIndexes as SupportEntryIndexes, EntryLookup, NonUniqueIndexes,
    },
};

#[derive(Clone, Copy, Debug)]
pub(super) enum EntryIndexes {
    Unique(ItemIndex),
    NonUnique {
        // Invariant: at least one index is Some, and indexes are not all the
        // same Some value.
        index1: Option<ItemIndex>,
        index2: Option<ItemIndex>,
    },
}

impl EntryIndexes {
    #[inline]
    pub(super) fn classify(
        index1: Option<ItemIndex>,
        index2: Option<ItemIndex>,
    ) -> EntryLookup<2> {
        SupportEntryIndexes::new([index1, index2]).classify()
    }

    #[inline]
    pub(super) fn from_non_unique(indexes: NonUniqueIndexes<2>) -> Self {
        let [index1, index2] = *indexes.indexes();
        EntryIndexes::NonUnique { index1, index2 }
    }

    #[inline]
    pub(super) fn is_unique(&self) -> bool {
        matches!(self, EntryIndexes::Unique(_))
    }

    #[inline]
    pub(super) fn disjoint_keys(&self) -> DisjointKeys<'_> {
        match self {
            EntryIndexes::Unique(index) => DisjointKeys::Unique(*index),
            EntryIndexes::NonUnique {
                index1: Some(index1),
                index2: Some(index2),
            } => {
                debug_assert_ne!(
                    index1, index2,
                    "index1 and index2 shouldn't match"
                );
                DisjointKeys::Key12([index1, index2])
            }
            EntryIndexes::NonUnique { index1: Some(index), index2: None } => {
                DisjointKeys::Key1(*index)
            }
            EntryIndexes::NonUnique { index1: None, index2: Some(index) } => {
                DisjointKeys::Key2(*index)
            }
            EntryIndexes::NonUnique { index1: None, index2: None } => {
                unreachable!("At least one index must be Some")
            }
        }
    }
}

pub(super) enum DisjointKeys<'a> {
    Unique(ItemIndex),
    Key1(ItemIndex),
    Key2(ItemIndex),
    Key12([&'a ItemIndex; 2]),
}
