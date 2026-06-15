/// Returns a symbolic `u8` constrained to `0 <= k < n`.
pub(crate) fn nondet_u8_below(n: u8) -> u8 {
    let k: u8 = soteria::nondet_bytes();
    soteria::assume(k < n);
    k
}

/// Length of the symbolic operation sequence for IdHashMap.
///
/// This is set to 3 because we have had a real bug that only reproduced with
/// three operations. More than that would be great but makes the proofs far too
/// slow to run.
pub(crate) const SEQ_OPS: usize = 3;

/// Size of the symbolic key domain.
pub(crate) const SEQ_KEYS: u8 = 2;
