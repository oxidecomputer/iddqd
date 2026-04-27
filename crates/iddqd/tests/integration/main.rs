mod bi_hash_map;
mod id_hash_map;
#[cfg(feature = "std")]
mod id_ord_map;
#[cfg(any(feature = "std", feature = "default-hasher"))]
mod panic_safety;
#[cfg(feature = "schemars08")]
mod schemars_tests;
#[cfg(all(
    feature = "std",
    feature = "default-hasher",
    target_pointer_width = "64",
    not(miri)
))]
mod size_tests;
mod tri_hash_map;
