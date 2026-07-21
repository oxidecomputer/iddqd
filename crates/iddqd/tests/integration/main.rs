mod bi_hash_map;
mod hegel_support;
mod id_hash_map;
#[cfg(feature = "std")]
mod id_ord_map;
#[cfg(all(feature = "std", feature = "default-hasher"))]
mod pathological;
#[cfg(feature = "schemars08")]
mod schemars_tests;
#[cfg(all(feature = "serde", feature = "std", feature = "default-hasher"))]
mod serde_size_hint;
#[cfg(all(
    feature = "std",
    feature = "default-hasher",
    target_pointer_width = "64",
    not(miri)
))]
mod size_tests;
mod tri_hash_map;
