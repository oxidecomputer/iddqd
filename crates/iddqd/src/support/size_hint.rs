use core::{cmp, mem};

// A mirror of
// https://github.com/serde-rs/serde/blob/7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8/serde_core/src/private/size_hint.rs#L12,
// used to cap size_hint-based preallocation to a reasonable value.
pub(crate) fn cautious<T>(hint: Option<usize>) -> usize {
    const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

    if mem::size_of::<T>() == 0 {
        0
    } else {
        cmp::min(hint.unwrap_or(0), MAX_PREALLOC_BYTES / mem::size_of::<T>())
    }
}
