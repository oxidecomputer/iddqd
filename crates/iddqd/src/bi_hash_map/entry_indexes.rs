use crate::support::ItemIndex;

#[derive(Clone, Copy, Debug)]
pub(super) enum EntryIndexes {
    Unique(ItemIndex),
    NonUnique {
        // Invariant: at least one index is Some, and indexes are different from
        // each other.
        index1: Option<ItemIndex>,
        index2: Option<ItemIndex>,
    },
}

impl EntryIndexes {
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
