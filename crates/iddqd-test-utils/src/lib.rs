pub mod borrowed_item;
pub mod eq_props;
pub mod naive_map;
#[cfg(feature = "serde")]
pub mod serde_utils;
pub mod test_item;
pub mod unwind;

/// Re-exports the `bumpalo` crate if the `allocator-api2` feature is enabled --
/// used by doctests.
#[cfg(feature = "allocator-api2")]
pub use bumpalo;
/// Re-exports `serde_json` if the `serde` feature is enabled -- used by
/// doctests.
#[cfg(feature = "serde")]
pub use serde_json;
