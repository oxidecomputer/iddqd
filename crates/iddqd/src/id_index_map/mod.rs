//! An index map where keys are part of the values and insertion order is preserved.
//!
//! For more information, see [`IdIndexMap`].

#[cfg(feature = "daft")]
mod daft_impls;
mod entry;
pub(crate) mod imp;
mod iter;
mod ref_mut;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;

pub use super::id_hash_map::IdHashItem;
#[cfg(feature = "daft")]
pub use daft_impls::Diff;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use imp::IdIndexMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
