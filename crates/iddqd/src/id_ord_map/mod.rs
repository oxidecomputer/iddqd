//! An ordered map where the keys are part of the values, based on a B-Tree.
//!
//! For more information, see [`IdOrdMap`].

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
pub use imp::IdOrdMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
pub use trait_defs::IdOrdItem;
