//! A hash map where keys are part of the values.
//!
//! For more information, see [`IdHashMap`].

#[cfg(feature = "daft")]
mod daft_impls;
mod entry;
pub(crate) mod imp;
mod iter;
mod ref_mut;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;
pub(crate) mod trait_defs;

#[cfg(feature = "daft")]
pub use daft_impls::Diff;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use imp::IdHashMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
pub use trait_defs::IdHashItem;
