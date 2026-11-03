//! External (black-box) contract proof for the FR-008 in-flight coalescer.
//!
//! FR: FR-008
//!
//! AC-008.2 in-flight request coalesce — external contract proof.
//! Covers the `SpeculativeCoalescer` protocol clauses: pending-group
//! registration on first poll, in-window callers sharing one execution,
//! post-window callers starting a fresh cycle, and per-key independence.
//!
//! This test lives OUTSIDE `crates/sharecli-core/src/speculation.rs` and
//! exercises only the public API surface:
//!
//! * `sharecli_core::speculation::SpeculativeCoalescer`
//! * `sharecli_ipc::CommandKey`
//!
//! It exists to close the validation gap that the in-crate unit test alone
//! cannot: that the documented protocol actually holds when driven from
//! outside the defining module, and that the documented *production* surface
//! (not just the test helper) is what is being exercised.
//!
//! Contract under test (from the `SpeculativeCoalescer` rustdoc):
//!
//! 1. The first caller registers a pending group and schedules a task that
//!    sleeps for `window`, then runs `exec()` once and broadcasts the result.
//! 2. Callers arriving during the window join the pending group as waiters
//!    and receive the broadcast result.
//! 3. Callers arriving after the window expiry find no pending group and
//!    start a fresh coalesce cycle.
//!
//! Each assertion below maps to one numbered protocol clause.

use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sharecli_core::speculation::SpeculativeCoalescer;
use sharecli_ipc::CommandKey;

/// Shared executor-invocation counter so we can assert `exec()` ran exactly
/// once for a group of callers inside one window.
#[derive(Debug, Default)]
struct ExecCounter {
    calls: AtomicU32,
}

impl ExecCounter {
    fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}

/// Build a closure that returns the given stdout and bumps the counter.
///
/// Mirrors the in-crate `exec_ok` helper but is defined OUTSIDE the crate, so
/// the test exercises the real public `coalesce` signature.
fn exec_ok(
    counter: Arc<ExecCounter>,
    stdout: &'static str,
) -> impl FnOnce() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = anyhow::Result<sharecli_ipc::CachedResult>> + Send>,
> + Send
       + 'static {
    move || {
        counter.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(sharecli_ipc::CachedResult {
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
                exit_code: 0,
            })
        })
    }
}

/// Assert `pending_len()` reaches `expected`, polling until it does.
///
/// `coalesce` is an `async fn` and registers its pending group on the FIRST
/// POLL of the returned future, so a freshly spawned task has to actually be
/// scheduled before the group is observable. A bare `yield_now().await`
/// assumes one scheduler hand-off always suffices; this helper instead polls
/// until the expected state is observed and gives up with a descriptive panic
/// after a fixed number of scheduler yields.
///
/// The bound is an ITERATION budget, not an elapsed-time deadline. These tests
/// run under `#[tokio::test(start_paused = true)]`, where the virtual clock
/// only advances while the runtime is idle, so a wall-clock deadline would
/// never expire (the wait would hang forever) and a `sleep`-driven loop could
/// silently push virtual time past the coalescing window. The loop therefore
/// yields (the only clock-independent way to hand the task queue back to the
/// runtime) and never sleeps.
///
/// `context` is the contract the assertion encodes; it is reported verbatim
/// when the bounded wait is exhausted so a timeout is as diagnosable as a plain
/// `assert_eq!` failure.
///
/// Deliberately duplicated in `crates/sharecli-core/src/speculation.rs`: this
/// is an integration test of the root `sharecli` package, which cannot reach
/// into `sharecli-core`'s `#[cfg(test)]` module without creating a circular
/// dev-dependency, and a new `src/` module would inflate the coverage
/// denominator for test-only code.
async fn assert_pending_len<F, Fut>(expected: usize, context: &str, read: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = usize>,
{
    const ATTEMPTS: usize = 1_024;
    let mut observed = 0usize;
    for _ in 0..ATTEMPTS {
        observed = read().await;
        if observed == expected {
            return;
        }
        // Hand the task queue back to the runtime so the spawned `coalesce`
        // task gets polled and can register its group.
        tokio::task::yield_now().await;
    }
    panic!(
        "bounded wait exhausted after {ATTEMPTS} scheduler yields: {context} \
         (expected pending_len {expected}, last observed {observed})"
    );
}

/// Protocol clause 1 (registration) + `pending_len` observability.
///
/// From OUTSIDE the crate: a coalesce cycle that is genuinely being driven
/// MUST register exactly one pending group, and that group MUST disappear
/// once the cycle resolves.
///
/// This is the property the in-crate regression test fixes. Reproducing it
/// from the public API proves the fix is a real behavioural property and not
/// an artifact of in-module access.
#[tokio::test(start_paused = true)]
async fn external_pending_len_reflects_active_cycle() {
    let coalescer = Arc::new(SpeculativeCoalescer::new(Duration::from_secs(1)));
    let key = CommandKey("external-active-cycle".into());

    assert_eq!(coalescer.pending_len().await, 0, "fresh coalescer MUST have no pending");

    let counter = Arc::new(ExecCounter::default());
    let cnt = Arc::clone(&counter);
    let k = key.clone();
    let c = Arc::clone(&coalescer);
    let handle = tokio::spawn(async move { c.coalesce(&k, exec_ok(cnt, "external")).await });

    // Wait (bounded) for the spawned task to register the pending group
    // rather than assuming a single scheduler yield is enough.
    assert_pending_len(
        1,
        "pending_len MUST show 1 active group while the window is still running",
        || async { coalescer.pending_len().await },
    )
    .await;

    // Advance virtual time past the window so the executor runs.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let result = handle.await.expect("join").expect("ok");
    assert_eq!(result.stdout, b"external");

    assert_eq!(
        coalescer.pending_len().await,
        0,
        "pending_len MUST drop to zero after the cycle resolves"
    );

    assert_eq!(counter.calls(), 1, "executor MUST be invoked exactly once");
}

/// Protocol clause 2 (joiners share one execution) + clause 3 (post-window
/// callers start a fresh cycle).
///
/// From OUTSIDE the crate: three callers inside one window share a single
/// executor invocation and receive the identical result; a caller arriving
/// after the window triggers a second, independent execution.
#[tokio::test(start_paused = true)]
async fn external_window_join_shares_one_execution_and_post_window_starts_fresh_cycle() {
    let coalescer = Arc::new(SpeculativeCoalescer::new(Duration::from_secs(1)));
    let key = CommandKey("external-window".into());
    let counter = Arc::new(ExecCounter::default());

    // --- Three callers inside one window ---
    let mut handles = Vec::new();
    for _ in 0..3 {
        let cnt = Arc::clone(&counter);
        let k = key.clone();
        let c = Arc::clone(&coalescer);
        handles.push(tokio::spawn(async move { c.coalesce(&k, exec_ok(cnt, "shared")).await }));
    }

    assert_pending_len(
        1,
        "three in-window callers MUST share ONE pending group, not three",
        || async { coalescer.pending_len().await },
    )
    .await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    for h in handles {
        let r = h.await.expect("join").expect("ok");
        assert_eq!(r.stdout, b"shared", "every in-window caller MUST receive the broadcast result");
    }

    assert_eq!(
        counter.calls(),
        1,
        "executor MUST be invoked exactly once for three in-window callers"
    );
    assert_eq!(coalescer.pending_len().await, 0, "pending group MUST be drained");

    // --- A caller arriving AFTER the window starts a fresh cycle ---
    let cnt = Arc::clone(&counter);
    let k = key.clone();
    let c = Arc::clone(&coalescer);
    let handle = tokio::spawn(async move { c.coalesce(&k, exec_ok(cnt, "fresh")).await });

    assert_pending_len(1, "post-window caller MUST register a FRESH pending group", || async {
        coalescer.pending_len().await
    })
    .await;

    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = handle.await.expect("join").expect("ok");
    assert_eq!(r.stdout, b"fresh");

    assert_eq!(
        counter.calls(),
        2,
        "post-window caller MUST trigger a second, independent execution"
    );
    assert_eq!(coalescer.pending_len().await, 0);
}

/// Different keys are independent groups.
#[tokio::test(start_paused = true)]
async fn external_distinct_keys_are_independent_groups() {
    let coalescer = Arc::new(SpeculativeCoalescer::new(Duration::from_secs(1)));
    let counter = Arc::new(ExecCounter::default());

    let mut handles = Vec::new();
    for name in ["alpha", "beta", "gamma"] {
        let cnt = Arc::clone(&counter);
        let k = CommandKey(name.into());
        let c = Arc::clone(&coalescer);
        handles.push(tokio::spawn(async move { c.coalesce(&k, exec_ok(cnt, "distinct")).await }));
    }

    assert_pending_len(
        3,
        "three distinct keys MUST register three independent pending groups",
        || async { coalescer.pending_len().await },
    )
    .await;

    tokio::time::sleep(Duration::from_secs(2)).await;
    for h in handles {
        h.await.expect("join").expect("ok");
    }

    assert_eq!(counter.calls(), 3, "each distinct key MUST get its own execution");
}
