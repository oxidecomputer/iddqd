# Agent instructions

## Testing

- Hegel state-machine invariants must always be declared `#[invariant(always_run)]`, never a plain `#[invariant]`. A plain invariant is only sampled between steps (about once per test case), so intermediate states would go unchecked.
