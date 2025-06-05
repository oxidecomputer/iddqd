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
#[cfg(feature = "schemars08")]
mod schemars_impls;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;
pub(crate) mod trait_defs;

#[cfg(feature = "daft")]
pub use daft_impls::{ByK1, ByK2, Diff, MapLeaf};
pub use entry::{
    Entry, OccupiedEntry, OccupiedEntryMut, OccupiedEntryRef, VacantEntry,
};
pub use imp::BiHashMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
pub use trait_defs::BiHashItem;
