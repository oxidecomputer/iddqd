use iddqd::{IdHashItem, IdOrdItem, id_upcast};

#[derive(Debug)]
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

#[derive(Debug)]
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

/// Inline payload size for the "large" record family. Sized to model a
/// realistic DB row's fixed-size fields (UUIDs, timestamps, enum
/// discriminants, packed flags) without dragging in heap allocations
/// that would dominate the measurement with allocator behavior.
pub const LARGE_RECORD_PAYLOAD: usize = 1024;

/// A `Record` with a 1 KiB inline payload, modeling a DB-row-sized
/// item stored by value in the map. Resizing the backing storage
/// memcpys this whole payload per element, so this type is the right
/// shape for measuring the cost of a `Vec` regrow on a populated map.
#[derive(Debug)]
pub struct RecordLargeOwnedU32 {
    pub index: u32,
    pub data: [u8; LARGE_RECORD_PAYLOAD],
}

impl IdHashItem for RecordLargeOwnedU32 {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.index
    }

    id_upcast!();
}

impl IdOrdItem for RecordLargeOwnedU32 {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.index
    }

    id_upcast!();
}

#[derive(Debug)]
pub struct RecordLargeBorrowedU32 {
    pub index: u32,
    pub data: [u8; LARGE_RECORD_PAYLOAD],
}

impl IdHashItem for RecordLargeBorrowedU32 {
    type Key<'a> = &'a u32;

    fn key(&self) -> Self::Key<'_> {
        &self.index
    }

    id_upcast!();
}

impl IdOrdItem for RecordLargeBorrowedU32 {
    type Key<'a> = &'a u32;

    fn key(&self) -> Self::Key<'_> {
        &self.index
    }

    id_upcast!();
}
