//! Symbolic-execution proof harnesses for iddqd internals that need
//! `pub(crate)` access (currently the [`ItemSet`] slot container).
//!
//! This module is compiled only under the [Soteria] symbolic-execution
//! frontend, which sets `--cfg soteria`.
//!
//! [Soteria]: https://soteria-tools.com

use crate::{
    internal::ValidateCompact,
    support::{ItemIndex, alloc::Global, item_set::ItemSet},
};

/// Returns a symbolic `u32` constrained to `0 <= k < n`.
fn nondet_index_below(n: u32) -> u32 {
    let k: u32 = soteria::nondet_bytes();
    soteria::assume(k < n);
    k
}

#[soteria::test]
fn item_set_insert_assigns_dense_indexes() {
    let mut set: ItemSet<u32, Global> = ItemSet::new();
    let a = set.assert_can_grow().insert(10);
    let b = set.assert_can_grow().insert(20);
    let c = set.assert_can_grow().insert(30);
    soteria::assert(a.as_u32() == 0, "first insert lands at index 0");
    soteria::assert(b.as_u32() == 1, "second insert lands at index 1");
    soteria::assert(c.as_u32() == 2, "third insert lands at index 2");
    soteria::assert(set.len() == 3, "three inserts give len 3");
    set.validate(ValidateCompact::Compact).expect("a hole-free set is compact");
}

/// The free-chain LIFO + structural-validity invariant: for any freed slot `k`,
/// removing it then inserting reuses slot `k`, preserves `len`, and keeps the
/// [`ItemSet`] invariant intact at every step.
#[soteria::test]
fn item_set_remove_then_insert_reuses_freed_slot() {
    let mut set: ItemSet<u32, Global> = ItemSet::new();
    set.assert_can_grow().insert(10);
    set.assert_can_grow().insert(20);
    set.assert_can_grow().insert(30);
    set.validate(ValidateCompact::Compact).expect("fresh set is compact");

    // Symbolic: which of the three occupied slots gets freed.
    let k = nondet_index_below(3);
    let removed = set.remove(ItemIndex::new(k));
    soteria::assert(removed.is_some(), "removed an occupied slot");
    soteria::assert(set.len() == 2, "remove decrements len");
    set.validate(ValidateCompact::NonCompact)
        .expect("one hole is a well-formed non-compact set");

    // LIFO reuse: the next insert lands back in the just-freed slot k.
    let reused = set.assert_can_grow().insert(99);
    soteria::assert(reused == ItemIndex::new(k), "freed slot k is reused");
    soteria::assert(set.len() == 3, "reinsert restores len");
    soteria::assert(
        set.get(ItemIndex::new(k)) == Some(&99),
        "the reused slot holds the new value",
    );
    set.validate(ValidateCompact::Compact)
        .expect("hole is filled, set is compact again");
}
