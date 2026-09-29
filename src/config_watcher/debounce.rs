//! Trailing-edge debounce state for [`crate::config_watcher`].
//!
//! Kept separate from the watcher so the deadline arithmetic — the part that
//! decides *when* a burst of file-system events becomes exactly one reload —
//! can be tested directly and deterministically with injected instants, rather
//! than only through wall-clock file-system timing.

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Debounce window: reload only after this much quiet following the latest
/// file-system event.
pub(super) const DEBOUNCE: Duration = Duration::from_millis(200);

/// Shared trailing-edge debounce state guarding both the bookkeeping and the
/// fire decision, so an event can never land between "deadline reached" and
/// "batch claimed".
#[derive(Debug)]
pub(super) struct Debouncer {
    /// Monotonic batch identity. Bumped on every observed event so a batch can
    /// be claimed only if nothing newer has arrived — an `Instant` timestamp
    /// alone could collide between two very close events.
    pub(super) generation: u64,
    /// Time of the most recent event. The deadline is measured from here, which
    /// is what makes the debounce trailing rather than leading.
    pub(super) last_event: Option<Instant>,
    /// True while events are outstanding and no reload has been sent for them.
    pub(super) pending: bool,
    /// Set on drop so the debounce thread exits instead of firing again.
    pub(super) shutdown: bool,
}

/// Coordination between the notify callback and the debounce thread.
pub(super) struct Shared {
    pub(super) state: Mutex<Debouncer>,
    pub(super) cv: Condvar,
}

impl Shared {
    pub(super) fn new() -> Self {
        Self { state: Mutex::new(Debouncer::new()), cv: Condvar::new() }
    }
}

impl Debouncer {
    pub(super) fn new() -> Self {
        Self { generation: 0, last_event: None, pending: false, shutdown: false }
    }

    /// Record an event at `now`, starting or extending the outstanding batch.
    pub(super) fn record(&mut self, now: Instant) {
        self.generation += 1;
        self.last_event = Some(now);
        self.pending = true;
    }

    /// When the outstanding batch becomes due: `DEBOUNCE` after the latest
    /// event. `None` when nothing has been observed.
    pub(super) fn due_at(&self) -> Option<Instant> {
        self.last_event.map(|t| t + DEBOUNCE)
    }

    /// Try to claim the batch identified by `generation` for firing.
    ///
    /// Returns `false` when a newer event has arrived (the caller must wait out
    /// the extended deadline) or when the watcher is shutting down.
    pub(super) fn claim(&mut self, generation: u64) -> bool {
        if self.shutdown || !self.pending || self.generation != generation {
            return false;
        }
        self.pending = false;
        true
    }
}

pub(super) fn lock(shared: &Shared) -> MutexGuard<'_, Debouncer> {
    shared.state.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn wait<'a>(
    shared: &'a Shared,
    guard: MutexGuard<'a, Debouncer>,
) -> MutexGuard<'a, Debouncer> {
    shared.cv.wait(guard).unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- trailing debounce ---

    /// The deadline must be measured from the **latest** event, not the first.
    /// A leading-edge scheme would set it from the first event and reload an
    /// intermediate state of the file.
    #[test]
    fn debounce_deadline_follows_the_latest_event() {
        let t0 = Instant::now();
        let mut d = Debouncer::new();

        d.record(t0);
        let after_first = d.due_at().expect("pending implies an event");
        assert_eq!(after_first, t0 + DEBOUNCE, "first event due one DEBOUNCE later");

        // A second event later in the window must push the deadline out.
        let t1 = t0 + Duration::from_millis(150);
        d.record(t1);
        let after_second = d.due_at().expect("pending implies an event");
        assert_eq!(after_second, t1 + DEBOUNCE, "deadline must extend with the latest event");
        assert!(after_second > after_first, "trailing deadline must move later, never earlier");
    }

    /// Once a newer event arrives, the older generation can no longer be
    /// claimed — the batch waits for the new deadline instead of firing twice.
    #[test]
    fn debounce_claim_rejects_a_stale_generation() {
        let t0 = Instant::now();
        let mut d = Debouncer::new();

        d.record(t0);
        let stale = d.generation;

        d.record(t0 + Duration::from_millis(10));
        assert!(
            !d.claim(stale),
            "a batch superseded by a newer event must not fire on the old deadline"
        );
        assert!(d.pending, "the newer event must keep the batch outstanding");

        let fresh = d.generation;
        assert!(d.claim(fresh), "the current generation must be claimable");
        assert!(!d.pending, "claiming must close the batch");
        assert!(!d.claim(fresh), "a claimed batch must not fire twice");
    }

    /// A batch whose deadline has already passed fires exactly once.
    #[test]
    fn debounce_fires_once_after_the_window() {
        let past = Instant::now() - Duration::from_millis(500);
        let mut d = Debouncer::new();
        d.record(past);

        let due = d.due_at().expect("pending implies an event");
        assert!(due <= Instant::now(), "a settled batch must already be due");

        let gen = d.generation;
        assert!(d.claim(gen), "a due batch must be claimable");
        assert!(!d.claim(gen), "the batch must fire only once");
    }

    /// Shutdown must win over any outstanding batch so the debounce thread
    /// exits rather than reloading during teardown.
    #[test]
    fn debounce_shutdown_prevents_firing() {
        let mut d = Debouncer::new();
        d.record(Instant::now() - Duration::from_millis(500));
        let gen = d.generation;
        d.shutdown = true;
        assert!(!d.claim(gen), "shutdown must suppress an outstanding reload");
    }
}
