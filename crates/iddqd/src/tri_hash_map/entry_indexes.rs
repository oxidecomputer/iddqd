use crate::support::{
    ItemIndex,
    entry::{
        DistinctIndexes, EntryIndexes as SupportEntryIndexes, EntryLookup,
        NonUniqueIndexes,
    },
};

#[derive(Clone, Copy, Debug)]
pub(super) enum EntryIndexes {
    Unique(ItemIndex),
    NonUnique(NonUniqueIndexes<3>),
}

impl EntryIndexes {
    #[inline]
    pub(super) fn classify(
        index1: Option<ItemIndex>,
        index2: Option<ItemIndex>,
        index3: Option<ItemIndex>,
    ) -> EntryLookup<3> {
        SupportEntryIndexes::new([index1, index2, index3]).classify()
    }

    #[inline]
    pub(super) fn from_non_unique(indexes: NonUniqueIndexes<3>) -> Self {
        EntryIndexes::NonUnique(indexes)
    }

    #[inline]
    pub(super) fn is_unique(&self) -> bool {
        matches!(self, EntryIndexes::Unique(_))
    }

    #[inline]
    pub(super) fn distinct(&self) -> Option<DistinctIndexes<3>> {
        match self {
            EntryIndexes::Unique(_) => None,
            EntryIndexes::NonUnique(indexes) => Some(indexes.distinct()),
        }
    }
}
