//! Shared scaffolding for panic safety tests.
//!
//! The map types invoke user-supplied `Hash`, `Eq`, and `Ord` impls on the key
//! type as part of every find/insert/remove. If one of those impls panics
//! partway through an operation, the items buffer and the external index tables
//! must remain consistent, otherwise a later operation could observe a stale or
//! out-of-bounds index.
//!
//! The panic safety tests use [`PanickyKey`] as the key type, and arm a
//! countdown via [`arm_panic_after`]; the next [`PanickyKey`] operation after
//! `n` successful ones panics. The usual shape is:
//!
//! 1. Fill the map with items.
//! 2. Probe how many key-trait calls the lookup path makes, typically by
//!    running a successful `contains_key` with [`take_op_count`].
//! 3. Arm the countdown so the panic fires on the call that follows the
//!    probe count (i.e. during the lookup that precedes the table mutation).
//! 4. Catch the panic and validate the map.
//!
//! We rely on nextest's process-per-test model to ensure that
//! [`PANIC_COUNTDOWN`] and [`OP_COUNT`] are not reused between tests running on
//! the same thread.

use core::{
    cell::Cell,
    cmp::Ordering,
    hash::{Hash, Hasher},
};

thread_local! {
    /// Counts down on each `PanickyKey` trait-method call. When zero,
    /// the next call panics. `u32::MAX` = disarmed.
    static PANIC_COUNTDOWN: Cell<u32> = const { Cell::new(u32::MAX) };
    /// Total number of `PanickyKey` trait-method calls observed; used
    /// by tests to probe how many calls a lookup path performs.
    static OP_COUNT: Cell<u32> = const { Cell::new(0) };
}

/// A key type whose `Hash`/`Eq`/`Ord` impls share a panic countdown, so that
/// tests can deterministically trigger a panic at a chosen point in any map
/// operation.
#[derive(Clone, Debug, Eq)]
pub(crate) struct PanickyKey(pub u32);

impl PanickyKey {
    fn observe_call(label: &'static str) {
        OP_COUNT.with(|c| c.set(c.get() + 1));
        PANIC_COUNTDOWN.with(|c| {
            let n = c.get();
            if n == 0 {
                panic!("PanickyKey::{label} panic triggered");
            }
            c.set(n - 1);
        });
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

/// Returns the operation counter and resets it to zero.
pub(crate) fn take_op_count() -> u32 {
    OP_COUNT.with(|c| c.replace(0))
}

/// Arms the countdown: the next `n` `PanickyKey` operations succeed,
/// and the call after that panics.
pub(crate) fn arm_panic_after(n: u32) {
    PANIC_COUNTDOWN.with(|c| c.set(n));
}

/// Disarms the countdown so subsequent `PanickyKey` operations don't
/// panic — call before any post-panic validation.
pub(crate) fn disarm_panic() {
    PANIC_COUNTDOWN.with(|c| c.set(u32::MAX));
}
