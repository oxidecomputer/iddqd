//! A hash map where values are uniquely indexed by two keys.
//!
//! For more information, see [`BiHashMap`].

#[cfg(feature = "daft")]
mod daft_impls;
mod entry;
mod entry_indexes;
pub(crate) mod imp;
mod iter;
mod ref_mut;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;
pub(crate) mod trait_defs;

#[cfg(feature = "daft")]
pub use daft_impls::Diff;
pub use entry::{
    Entry, OccupiedEntry, OccupiedEntryMut, OccupiedEntryRef, VacantEntry,
};
pub use imp::BiHashMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
pub use trait_defs::BiHashItem;
