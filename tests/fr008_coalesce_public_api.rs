//! External (black-box) contract proof for the FR-008 in-flight coalescer.
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

    // Let the spawned task register the pending group.
    tokio::task::yield_now().await;

    assert_eq!(
        coalescer.pending_len().await,
        1,
        "pending_len MUST show 1 active group while the window is still running"
    );

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

    tokio::task::yield_now().await;
    assert_eq!(
        coalescer.pending_len().await,
        1,
        "three in-window callers MUST share ONE pending group, not three"
    );

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

    tokio::task::yield_now().await;
    assert_eq!(
        coalescer.pending_len().await,
        1,
        "post-window caller MUST register a FRESH pending group"
    );

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

    tokio::task::yield_now().await;
    assert_eq!(
        coalescer.pending_len().await,
        3,
        "three distinct keys MUST register three independent pending groups"
    );

    tokio::time::sleep(Duration::from_secs(2)).await;
    for h in handles {
        h.await.expect("join").expect("ok");
    }

    assert_eq!(counter.calls(), 3, "each distinct key MUST get its own execution");
}
