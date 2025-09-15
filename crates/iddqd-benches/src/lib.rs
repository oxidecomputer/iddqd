use iddqd::{IdHashItem, IdOrdItem, id_upcast};

pub struct RecordOwnedU32 {
    pub index: u32,
    pub data: String,
}

impl IdHashItem for RecordOwnedU32 {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.index
    }

    id_upcast!();
}

impl IdOrdItem for RecordOwnedU32 {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.index
    }

    id_upcast!();
}

pub struct RecordBorrowedU32 {
    pub index: u32,
    pub data: String,
}

impl IdHashItem for RecordBorrowedU32 {
    type Key<'a> = &'a u32;

    fn key(&self) -> Self::Key<'_> {
        &self.index
    }

    id_upcast!();
}

impl IdOrdItem for RecordBorrowedU32 {
    type Key<'a> = &'a u32;

    fn key(&self) -> Self::Key<'_> {
        &self.index
    }

    id_upcast!();
}
