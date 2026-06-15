/// Returns a symbolic `u8` constrained to `0 <= k < n`.
pub(crate) fn nondet_u8_below(n: u8) -> u8 {
    let k: u8 = soteria::nondet_bytes();
    soteria::assume(k < n);
    k
}

/// Length of the symbolic operation sequence for `IdHashMap` and `BiHashMap`.
///
/// This is set to 3 because we have had a real bug that only reproduced with
/// three operations. More than that would be great but makes the proofs far too
/// slow to run.
pub(crate) const SEQ_OPS: usize = 3;

/// Length of the symbolic operation sequence for `TriHashMap`.
///
/// Depth 3 is far too slow to run in CI, so we use depth 2 instead. (This is a
/// real limitation with model checking, since that exhaustively explores the
/// state space.)
///
/// [`TriHashMap`]: iddqd::TriHashMap
pub(crate) const TRI_SEQ_OPS: usize = 2;

/// Size of the symbolic key domain.
pub(crate) const SEQ_KEYS: u8 = 2;
