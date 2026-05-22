//! Shared scaffolding for panic safety tests.
//!
//! [`PanickyKey`] is a key whose `Hash`/`Eq`/`Ord`/`Drop` impls share a
//! thread-local panic countdown. The call after `n` successful ones panics.
//! Drop is included so the harness also exercises the post-`prepare_*`,
//! pre-commit windows in `IdOrdMap` (and analogous windows in the hash
//! maps) where a user-key Drop is the only thing that could panic. Each
//! proptest drives a random sequence of [`PanickyOp`]s, and after every
//! step asserts:
//!
//! * `validate()`
//! * a `contains_key` round-trip on every surviving item
//! * (for atomic ops that panicked) that the post-op state equals the pre-op
//!   snapshot.

use core::{
    cell::Cell,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};
use equivalent::{Comparable, Equivalent};
use iddqd_test_utils::unwind::catch_panic;
use proptest::prelude::*;

thread_local! {
    static PANIC_COUNTDOWN: Cell<Option<u32>> = const { Cell::new(None) };
    static OP_COUNT: Cell<u32> = const { Cell::new(0) };
}

/// A key whose `Hash`/`Eq`/`Ord`/`Drop` impls share a panic countdown,
/// so tests can deterministically trigger a panic at a chosen point.
#[derive(Clone, Debug, Eq)]
pub(crate) struct PanickyKey(pub u32);

impl PanickyKey {
    fn observe_call(label: &'static str) {
        PANIC_COUNTDOWN.with(|c| {
            // When disarmed, don't tick `OP_COUNT`. This matters for two
            // distinct cases:
            //
            // * Calls outside `run_armed` (validation, assertions): they
            //   shouldn't count toward the next armed step.
            // * Calls during panic unwinding *after* the countdown has fired:
            //   key drops along the unwind path would otherwise tick
            //   `OP_COUNT` past `n + 1` and break
            //   `assert_panic_fired_as_expected`.
            let Some(n) = c.get() else { return };
            OP_COUNT.with(|c| c.set(c.get() + 1));
            if n == 0 {
                // Disarm before panicking so additional `observe_call`s
                // during unwinding (notably key drops) don't double-panic.
                c.set(None);
                panic!("PanickyKey::{label} panic triggered");
            }
            c.set(Some(n - 1));
        });
    }
}

impl Drop for PanickyKey {
    fn drop(&mut self) {
        Self::observe_call("drop");
    }
}

/// Caller-side lookup key.
///
/// Shares `PanickyKey`'s countdown for `Hash`/`Eq`/`Ord` so the harness
/// still exercises mid-op panics from comparator calls. Deliberately does
/// **not** panic from `Drop`: a search key is constructed by the caller,
/// passed by reference into a map op, and dropped *after* the op returns
/// (per Rust temporary lifetime rules). A `Drop` panic at that point isn't
/// within the map's atomic-op window, so observing it would conflate
/// post-op cleanup with mid-op failures and spuriously flag atomic ops as
/// violating their invariant.
#[derive(Clone, Debug)]
pub(crate) struct PanickySearchKey(pub u32);

impl Hash for PanickySearchKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        PanickyKey::observe_call("search-hash");
        self.0.hash(state);
    }
}

impl Equivalent<PanickyKey> for PanickySearchKey {
    fn equivalent(&self, key: &PanickyKey) -> bool {
        PanickyKey::observe_call("search-equivalent");
        self.0 == key.0
    }
}

impl Comparable<PanickyKey> for PanickySearchKey {
    fn compare(&self, key: &PanickyKey) -> Ordering {
        PanickyKey::observe_call("search-compare");
        self.0.cmp(&key.0)
    }
}

impl PartialEq for PanickyKey {
    fn eq(&self, other: &Self) -> bool {
        Self::observe_call("eq");
        self.0 == other.0
    }
}

impl PartialOrd for PanickyKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PanickyKey {
    fn cmp(&self, other: &Self) -> Ordering {
        Self::observe_call("cmp");
        self.0.cmp(&other.0)
    }
}

impl Hash for PanickyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::observe_call("hash");
        self.0.hash(state);
    }
}

fn take_op_count() -> u32 {
    OP_COUNT.with(|c| c.replace(0))
}

pub(crate) fn arm_panic_after(n: u32) {
    PANIC_COUNTDOWN.with(|c| c.set(Some(n)));
}

pub(crate) fn disarm_panic() {
    PANIC_COUNTDOWN.with(|c| c.set(None));
}

#[derive(Debug)]
pub(crate) struct PanickyOp<A> {
    pub(crate) action: A,
    pub(crate) armed: Option<u32>,
}

impl<A> Arbitrary for PanickyOp<A>
where
    A: Arbitrary + fmt::Debug + 'static,
{
    type Parameters = A::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(args: A::Parameters) -> Self::Strategy {
        // Bias towards `None` so the map fills up, otherwise panicking ops
        // would dominate and leave the map empty.
        let armed = prop_oneof![
            7 => Just(None),
            3 => (0..16_u32).prop_map(Some),
        ];
        (any_with::<A>(args), armed)
            .prop_map(|(action, armed)| PanickyOp { action, armed })
            .boxed()
    }
}

/// Run `f` with the panic countdown set, then unconditionally disarm
/// so a leftover countdown can't trip later code. Returns
/// `(panicked, ops)` where `ops` is the count of `PanickyKey`
/// trait-method calls made during `f`.
pub(crate) fn run_armed(armed: Option<u32>, f: impl FnOnce()) -> (bool, u32) {
    let _ = take_op_count();
    if let Some(n) = armed {
        arm_panic_after(n);
    }
    let result = catch_panic(f);
    disarm_panic();
    let ops = take_op_count();
    (result.is_none(), ops)
}

/// Asserts that the panic-countdown infrastructure fired (or didn't)
/// exactly as the arming would predict.
///
/// "Key call" here means any of `Hash`/`Eq`/`Ord`/`Drop` on `PanickyKey`.
/// With `armed = Some(n)`, the panic should fire on the `(n+1)`-th key
/// call, so `panicked` implies `ops == n + 1`, and `!panicked` implies
/// the action made at most `n` key calls. With `armed = None`, no
/// panic should escape.
pub(crate) fn assert_panic_fired_as_expected(
    op_label: &dyn fmt::Display,
    armed: Option<u32>,
    panicked: bool,
    ops: u32,
) {
    match (armed, panicked) {
        (Some(n), true) => assert_eq!(
            ops,
            n + 1,
            "op {op_label} (armed: {n}) panicked on key call {ops}, \
             expected call {}",
            n + 1,
        ),
        (Some(n), false) => assert!(
            ops <= n,
            "op {op_label} (armed: {n}) made {ops} key call(s) but \
             did not panic — the panic countdown failed to fire",
        ),
        (None, true) => panic!(
            "op {op_label} panicked unexpectedly with no armed \
             countdown (ops: {ops})",
        ),
        (None, false) => {}
    }
}

/// `K` is a single key for `IdHashMap`/`IdOrdMap` or a tuple of all
/// keys for `BiHashMap`/`TriHashMap`.
pub(crate) fn sorted_keys<I, K, F>(items: I, key_of: F) -> Vec<K>
where
    I: IntoIterator,
    K: Ord,
    F: Fn(I::Item) -> K,
{
    let mut keys: Vec<K> = items.into_iter().map(key_of).collect();
    keys.sort_unstable();
    keys
}

/// Classifies how an action should behave under a user-trait panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanicSafety {
    /// A panic must leave the map in its pre-call state.
    Atomic,
    /// Composed of atomic sub-steps; a panic may leave the map in a different
    /// but still-valid state.
    StepAtomic,
    /// May corrupt the underlying table. Callers must skip arming a panic for
    /// this op.
    #[allow(dead_code)] // unused without `default-hasher`
    MayCorruptOnPanic,
}

/// Asserts that every surviving item is findable, and (for atomic ops
/// that panicked) that the post-op state equals the pre-op snapshot.
///
/// `contains_keys` should check *all* of an item's keys for multi-key
/// maps.
#[expect(clippy::too_many_arguments)]
pub(crate) fn assert_post_op_invariants<K>(
    step: usize,
    op_label: &dyn fmt::Display,
    armed: Option<u32>,
    panicked: bool,
    panic_safety: PanicSafety,
    pre_state: &[K],
    post_state: &[K],
    contains_keys: impl Fn(&K) -> bool,
) where
    K: PartialEq + fmt::Debug,
{
    for key in post_state {
        assert!(
            contains_keys(key),
            "item with key {key:?} not findable after op {step} \
             ({op_label}, armed: {armed:?}, panicked: {panicked})",
        );
    }
    if panicked && panic_safety == PanicSafety::Atomic {
        assert_eq!(
            post_state, pre_state,
            "atomic op {op_label} (armed: {armed:?}) panicked at \
             step {step} but the map state changed",
        );
    }
}
