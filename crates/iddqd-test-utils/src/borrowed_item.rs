#[cfg(feature = "std")]
use iddqd::IdOrdItem;
use iddqd::{
    BiHashItem, IdHashItem, TriHashItem, bi_upcast, id_upcast, tri_upcast,
};
use std::{borrow::Cow, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BorrowedItem<'a> {
    pub key1: &'a str,
    pub key2: Cow<'a, [u8]>,
    pub key3: &'a Path,
}

impl<'a> IdHashItem for BorrowedItem<'a> {
    type Key<'k>
        = &'a str
    where
        Self: 'k;

    fn key(&self) -> Self::Key<'_> {
        self.key1
    }

    id_upcast!();
}

#[cfg(feature = "std")]
impl<'a> IdOrdItem for BorrowedItem<'a> {
    type Key<'k>
        = &'a str
    where
        Self: 'k;

    fn key(&self) -> Self::Key<'_> {
        self.key1
    }

    id_upcast!();
}

impl<'a> BiHashItem for BorrowedItem<'a> {
    type K1<'k>
        = &'a str
    where
        Self: 'k;
    type K2<'k>
        = &'k [u8]
    where
        Self: 'k;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        &*self.key2
    }

    bi_upcast!();
}

impl<'a> TriHashItem for BorrowedItem<'a> {
    type K1<'k>
        = &'a str
    where
        Self: 'k;
    type K2<'k>
        = &'k [u8]
    where
        Self: 'k;
    type K3<'k>
        = &'a Path
    where
        Self: 'k;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        &*self.key2
    }

    fn key3(&self) -> Self::K3<'_> {
        self.key3
    }

    tri_upcast!();
}
