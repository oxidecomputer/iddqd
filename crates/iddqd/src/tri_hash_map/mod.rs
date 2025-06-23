//! A hash map where values are uniquely indexed by three keys.
//!
//! For more information, see [`TriHashMap`].

#[cfg(feature = "daft")]
mod daft_impls;
pub(crate) mod imp;
mod iter;
#[cfg(feature = "proptest")]
mod proptest_impls;
mod ref_mut;
#[cfg(feature = "schemars08")]
mod schemars_impls;
#[cfg(feature = "serde")]
mod serde_impls;
mod tables;
pub(crate) mod trait_defs;

#[cfg(feature = "daft")]
pub use daft_impls::{ByK1, ByK2, ByK3, Diff, MapLeaf};
pub use imp::TriHashMap;
pub use iter::{IntoIter, Iter, IterMut};
#[cfg(all(feature = "proptest", feature = "default-hasher"))]
pub use proptest_impls::prop_strategy;
#[cfg(feature = "proptest")]
pub use proptest_impls::{
    TriHashMapStrategy, TriHashMapValueTree, prop_strategy_with_hasher,
    prop_strategy_with_hasher_in,
};
pub use ref_mut::RefMut;
pub use trait_defs::TriHashItem;
