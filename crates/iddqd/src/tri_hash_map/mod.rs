//! A hash map where values are uniquely indexed by three keys.
//!
//! TODO: expand on this

pub(crate) mod imp;
mod iter;
mod ref_mut;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;
pub(crate) mod trait_defs;

pub use imp::TriHashMap;
pub use iter::{IntoIter, Iter, IterMut};
pub use ref_mut::RefMut;
pub use trait_defs::TriHashItem;
