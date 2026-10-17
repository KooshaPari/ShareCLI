//! FR-008 — speculative execution & speculative coalesce.
//!
//! This module hosts two related, complementary mechanisms:
//!
//! - [`SpeculationTracker`] (long-running): records command-frequency
//!   histograms in-process. When a command crosses the speculation
//!   threshold the background task pre-executes it during idle periods
//!   and stores the result in the [`CoalesceCache`], so the next real
//!   `Hypervisor::run` call is a cache hit.
//! - [`SpeculativeCoalescer`] (per-call): a thread-safe in-flight
//!   request deduplicator. Concurrent identical calls within
//!   [`SPECULATIVE_COALESCE_WINDOW`] are merged into a single execution
//!   that broadcasts the same `Result<CachedResult>` to all callers.
//!
//! # Design constraints
//!
//! - Only **read-only** (non-nocache) commands are speculated on.
//! - Speculation respects the [`ThermalGate`] — no pre-execution when the
//!   device is thermally throttled.
//! - The background task is best-effort: failures are logged and swallowed.
//! - A sliding-window counter prevents stale高频 commands from being
//!   speculated on indefinitely.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use sharecli_ipc::{CachedResult, CoalesceCache, CommandKey};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How many cache hits within the window are needed before a command becomes
/// a speculation candidate.
pub const SPECULATION_THRESHOLD: u32 = 3;

/// Sliding window duration for the frequency counter.
pub const SPECULATION_WINDOW: Duration = Duration::from_secs(300);

/// Maximum number of distinct commands to speculate on per background cycle.
pub const SPECULATION_MAX_CANDIDATES: usize = 5;

/// Interval between background speculation cycles.
pub const SPECULATION_INTERVAL: Duration = Duration::from_secs(30);

/// Default in-flight coalesce window for [`SpeculativeCoalescer`].
///
/// Concurrent identical calls arriving within this window are merged
/// into a single execution. Tuned to be long enough to absorb a typical
/// fan-out burst (a few hundred µs of I/O scheduling jitter) but short
/// enough to avoid holding the executor hostage for a noticeable period.
pub const SPECULATIVE_COALESCE_WINDOW: Duration = Duration::from_millis(100);

// ---------------------------------------------------------------------------
// SpeculationTracker
// ---------------------------------------------------------------------------

/// A request to pre-execute a command during an idle period.
#[derive(Debug, Clone)]
pub struct SpeculationCandidate {
    pub key: CommandKey,
    pub argv: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub env: Vec<(String, String)>,
}

/// In-memory command-frequency tracker.
///
/// Wrapped in `Arc<Mutex<…>>` so the background task can drain candidates
/// without blocking the hot `Hypervisor::run` path.
#[derive(Default)]
struct Inner {
    /// CommandKey → (hit count, first-seen instant).
    hits: HashMap<String, (u32, Instant)>,
    /// CommandKey → request details needed for re-execution.
    requests: HashMap<String, SpeculationCandidate>,
}

#[derive(Default)]
pub struct SpeculationTracker {
    inner: Mutex<Inner>,
}

impl SpeculationTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one cache hit for `key`.
    ///
    /// Called on every successful coalesce lookup in [`Hypervisor::run`].
    /// The caller supplies the original [`SpawnRequest`] details so the
    /// background task can replay the command later.
    pub async fn record_hit(
        &self,
        key: &CommandKey,
        argv: &[String],
        cwd: &std::path::Path,
        env: &[(String, String)],
    ) {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();
        let entry = inner.hits.entry(key.0.clone()).or_insert((0, now));
        entry.0 += 1;
        // Refresh window start on each hit so the counter slides.
        entry.1 = now;

        // Store request details if not already present (first hit).
        inner.requests.entry(key.0.clone()).or_insert_with(|| SpeculationCandidate {
            key: key.clone(),
            argv: argv.to_vec(),
            cwd: cwd.to_path_buf(),
            env: env.to_vec(),
        });
    }

    /// Drain the top-N speculation candidates whose hit count ≥ threshold
    /// and whose window has not expired.
    ///
    /// The returned candidates are removed from the tracker so they are
    /// not speculated on again until they accumulate fresh hits.
    pub async fn drain_candidates(&self) -> Vec<SpeculationCandidate> {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();

        // Filter to candidates above threshold within the sliding window.
        let mut scored: Vec<(u32, String)> = inner
            .hits
            .iter()
            .filter(|(_, (count, first))| {
                *count >= SPECULATION_THRESHOLD && now.duration_since(*first) <= SPECULATION_WINDOW
            })
            .map(|(key, (count, _))| (*count, key.clone()))
            .collect();

        // Highest frequency first, then stable key order for deterministic truncation.
        scored.sort_by(|(left_count, left_key), (right_count, right_key)| {
            right_count.cmp(left_count).then_with(|| left_key.cmp(right_key))
        });
        scored.truncate(SPECULATION_MAX_CANDIDATES);

        let mut candidates = Vec::new();
        for (_, key) in scored {
            if let Some(candidate) = inner.requests.remove(&key) {
                candidates.push(candidate);
            }
            // Reset the counter so we don't re-speculate immediately.
            inner.hits.remove(&key);
        }

        candidates
    }

    /// Number of tracked commands (for diagnostics / tests).
    pub async fn len(&self) -> usize {
        self.inner.lock().await.hits.len()
    }

    /// Whether the tracker is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.hits.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SpeculativeCoalescer (FR-008 in-flight request coalesce)
// ---------------------------------------------------------------------------

/// In-flight group of callers waiting on one executor invocation.
struct PendingCoalesce {
    waiters: Vec<oneshot::Sender<Result<CachedResult>>>,
}

/// Inner state for [`SpeculativeCoalescer`]: a key → pending-group map.
#[derive(Default)]
struct CoalesceInner {
    pending: HashMap<String, PendingCoalesce>,
}

/// Thread-safe in-flight speculative coalescer.
///
/// Multiple [`coalesce`][SpeculativeCoalescer::coalesce] calls for the same
/// [`CommandKey`] arriving within [`window`][SpeculativeCoalescer::window]
/// are merged into a single execution. All callers in the window receive the
/// same [`Result<CachedResult>`] from the first-registered executor.
///
/// # Protocol
///
/// 1. The first caller registers a pending group and schedules a Tokio task
///    that sleeps for `window`, then runs `exec()` once and broadcasts the
///    result to every waiter.
/// 2. Callers arriving during the window join the pending group as waiters
///    and receive the broadcast result.
/// 3. Callers arriving after the window expiry find no pending group and
///    start a fresh coalesce cycle.
///
/// # Thread safety
///
/// The pending map is guarded by a [`tokio::sync::Mutex`]. The mutex is held
/// only briefly during registration / drainage — never across the `await`
/// of the executor. `Arc<SpeculativeCoalescer>` may be shared freely across
/// tasks and threads.
pub struct SpeculativeCoalescer {
    window: Duration,
    inner: Arc<Mutex<CoalesceInner>>,
}

impl Default for SpeculativeCoalescer {
    fn default() -> Self {
        Self::new(SPECULATIVE_COALESCE_WINDOW)
    }
}

impl SpeculativeCoalescer {
    /// Create a coalescer with the given window.
    pub fn new(window: Duration) -> Self {
        Self { window, inner: Arc::new(Mutex::new(CoalesceInner::default())) }
    }

    /// Create a coalescer with [`SPECULATIVE_COALESCE_WINDOW`].
    pub fn with_default_window() -> Self {
        Self::default()
    }

    /// The configured coalesce window.
    pub fn window(&self) -> Duration {
        self.window
    }

    /// Number of currently-pending coalesce groups (for tests/diagnostics).
    pub async fn pending_len(&self) -> usize {
        self.inner.lock().await.pending.len()
    }

    /// Run or join an in-flight speculative coalesce for `key`.
    ///
    /// `exec` is invoked at most once per coalesce window. All callers
    /// within the window receive the same `Result<CachedResult>` from the
    /// first-registered executor.
    ///
    /// Callers other than the leader still receive a `Result<CachedResult>`
    /// even though their `exec` closure is dropped on the floor.
    pub async fn coalesce<F, Fut>(&self, key: &CommandKey, exec: F) -> Result<CachedResult>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<CachedResult>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel::<Result<CachedResult>>();
        let mut is_leader = false;

        // Register as a waiter (or first caller / leader) under the lock.
        {
            let mut guard = self.inner.lock().await;
            match guard.pending.entry(key.0.clone()) {
                std::collections::hash_map::Entry::Occupied(mut occ) => {
                    occ.get_mut().waiters.push(tx);
                }
                std::collections::hash_map::Entry::Vacant(vac) => {
                    vac.insert(PendingCoalesce { waiters: vec![tx] });
                    is_leader = true;
                }
            }
        }

        if is_leader {
            let key_owned = key.0.clone();
            let inner = Arc::clone(&self.inner);
            let window = self.window;
            tokio::spawn(async move {
                // Wait out the window so other callers can join the group.
                tokio::time::sleep(window).await;

                // Drain the pending group; everyone who arrived during the
                // window is in `waiters`. Anyone arriving AFTER this drain
                // will start a fresh coalesce cycle — correct, by design.
                let waiters = {
                    let mut guard = inner.lock().await;
                    guard.pending.remove(&key_owned).map(|g| g.waiters).unwrap_or_default()
                };

                // Run the executor once and broadcast the result.
                let result = exec().await;
                for waiter in waiters {
                    // Receiver may have been dropped — that's fine, the
                    // caller moved on. Ignore the send error.
                    let _ = waiter.send(clone_result(&result));
                }
            });
        }

        rx.await
            .map_err(|_| anyhow!("speculative coalesce: executor task dropped before delivering result"))?
    }
}

/// Clone an [`anyhow::Error`] by formatting it into a new one.
///
/// We can't `Clone` `anyhow::Error` directly, so we re-format the chain.
fn clone_result(result: &Result<CachedResult>) -> Result<CachedResult> {
    match result {
        Ok(v) => Ok(v.clone()),
        Err(e) => Err(anyhow!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// Background speculation task
// ---------------------------------------------------------------------------

/// Spawn a background task that periodically pre-executes high-frequency
/// commands into the coalesce cache.
///
/// The task is best-effort: it logs and swallows any errors.  It does NOT
/// compete with real requests — it only runs during idle periods and
/// respects the thermal gate.
pub fn spawn_speculation_task(
    tracker: Arc<SpeculationTracker>,
    cache: CoalesceCache,
    thermal_gate: Arc<dyn crate::ThermalGate>,
) {
    // Best-effort background task. The hypervisor constructor may run outside
    // a Tokio runtime (sync CLI wiring, unit tests); without a reactor there
    // is nothing to spawn onto, so skip silently rather than panic.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SPECULATION_INTERVAL).await;

            // Respect thermal gate — never speculate when throttled.
            match thermal_gate.check() {
                crate::ThermalDecision::Allow => {}
                crate::ThermalDecision::Warn => {
                    debug!("speculation: thermal Warn — skipping cycle");
                    continue;
                }
                crate::ThermalDecision::Refuse => {
                    debug!("speculation: thermal Refuse — skipping cycle");
                    continue;
                }
            }

            let candidates = tracker.drain_candidates().await;
            if candidates.is_empty() {
                continue;
            }

            info!(count = candidates.len(), "speculation: pre-executing candidates");

            for candidate in candidates {
                // Skip if the cache already has a fresh entry.
                match cache.lookup(&candidate.key) {
                    Ok(Some(_)) => {
                        debug!(key = %candidate.key.0, "speculation: cache already warm — skip");
                        continue;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!(key = %candidate.key.0, err = %e, "speculation: lookup failed");
                        continue;
                    }
                }

                // Pre-execute the command.
                let argv = candidate.argv.clone();
                let cwd = candidate.cwd.clone();
                let env = candidate.env.clone();

                let result =
                    tokio::task::spawn_blocking(move || speculate_execute(&argv, &cwd, &env)).await;

                match result {
                    Ok(Ok(cached)) => {
                        if let Err(e) = cache.store(&candidate.key, &cached) {
                            warn!(
                                key = %candidate.key.0,
                                err = %e,
                                "speculation: cache store failed"
                            );
                        } else {
                            debug!(
                                key = %candidate.key.0,
                                exit = cached.exit_code,
                                "speculation: pre-execute + store ok"
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            key = %candidate.key.0,
                            err = %e,
                            "speculation: execute failed"
                        );
                    }
                    Err(e) => {
                        error!(
                            key = %candidate.key.0,
                            err = %e,
                            "speculation: spawn_blocking panicked"
                        );
                    }
                }
            }
        }
    });
}

/// Synchronously execute a command and capture its output.
///
/// Used inside `spawn_blocking` so the tokio runtime is not blocked by
/// long-running processes.
fn speculate_execute(
    argv: &[String],
    cwd: &std::path::Path,
    env: &[(String, String)],
) -> Result<CachedResult> {
    let (program, args) =
        argv.split_first().ok_or_else(|| anyhow::anyhow!("speculate: argv is empty"))?;

    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .map_err(|e| anyhow::anyhow!("speculate: failed to spawn {:?}: {e}", argv))?;

    Ok(CachedResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn tracker_records_hits_and_drains() {
        let tracker = SpeculationTracker::new();
        let key = CommandKey("abc123".into());
        let cwd = std::path::PathBuf::from("/tmp");
        let argv = vec!["echo".into(), "hello".into()];

        // Below threshold — no candidates.
        for _ in 0..2 {
            tracker.record_hit(&key, &argv, &cwd, &[]).await;
        }
        assert!(tracker.drain_candidates().await.is_empty());

        // Cross threshold.
        tracker.record_hit(&key, &argv, &cwd, &[]).await;
        let candidates = tracker.drain_candidates().await;
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].key, key);
        assert_eq!(candidates[0].argv, argv);

        // Drained — empty again.
        assert!(tracker.drain_candidates().await.is_empty());
    }

    #[tokio::test]
    async fn tracker_respects_max_candidates() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");

        // Insert more than SPECULATION_MAX_CANDIDATES above threshold.
        for i in 0..(SPECULATION_MAX_CANDIDATES + 3) {
            let key = CommandKey(format!("key-{i:04}"));
            let argv = vec![format!("cmd-{i}")];
            for _ in 0..SPECULATION_THRESHOLD {
                tracker.record_hit(&key, &argv, &cwd, &[]).await;
            }
        }

        let candidates = tracker.drain_candidates().await;
        assert_eq!(candidates.len(), SPECULATION_MAX_CANDIDATES, "must cap candidates");
        let keys: Vec<_> = candidates.into_iter().map(|candidate| candidate.key.0).collect();
        let expected: Vec<_> =
            (0..SPECULATION_MAX_CANDIDATES).map(|i| format!("key-{i:04}")).collect();
        assert_eq!(keys, expected, "equal counts must use key order before truncation");
    }

    #[tokio::test]
    async fn tracker_empty_after_drain() {
        let tracker = SpeculationTracker::new();
        assert!(tracker.is_empty().await);

        let key = CommandKey("xyz".into());
        let cwd = std::path::PathBuf::from("/tmp");
        let argv = vec!["ls".into()];

        for _ in 0..SPECULATION_THRESHOLD {
            tracker.record_hit(&key, &argv, &cwd, &[]).await;
        }
        assert!(!tracker.is_empty().await);

        tracker.drain_candidates().await;
        assert!(tracker.is_empty().await);
    }

    #[test]
    fn speculate_execute_echo() {
        let argv = vec!["echo".into(), "spec-test".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]).expect("execute");
        assert_eq!(result.exit_code, 0);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("spec-test"));
    }

    #[test]
    fn speculate_execute_empty_argv_returns_err() {
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&[], &cwd, &[]);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("argv is empty"),
            "error MUST mention empty argv"
        );
    }

    #[test]
    fn speculate_execute_nonexistent_command_returns_error() {
        let argv = vec!["__nonexistent_binary_12345__".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("failed to spawn"),
            "error MUST mention spawn failure"
        );
    }

    #[test]
    fn speculate_execute_captures_stderr() {
        // `echo` to stderr via sh -c
        let argv = vec!["sh".into(), "-c".into(), "echo err-msg >&2".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]).expect("execute");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("err-msg"), "stderr MUST be captured");
    }

    #[test]
    fn speculate_execute_nonzero_exit_code() {
        let argv = vec!["sh".into(), "-c".into(), "exit 42".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]).expect("execute");
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn speculate_execute_with_env_vars() {
        let argv = vec!["sh".into(), "-c".into(), "echo $MY_SPEC_VAR".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let env = vec![("MY_SPEC_VAR".into(), "hello_env".into())];
        let result = speculate_execute(&argv, &cwd, &env).expect("execute");
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("hello_env"), "env vars MUST be passed");
    }

    #[test]
    fn speculate_execute_with_cwd() {
        let argv = vec!["pwd".into()];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]).expect("execute");
        let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
        // On macOS, /var may resolve to /private/var via symlink
        assert!(stdout == "/tmp" || stdout == "/private/tmp", "cwd MUST be /tmp; got: {stdout}");
    }

    #[tokio::test]
    async fn tracker_len_matches_inserted_keys() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");

        let key_a = CommandKey("aaa".into());
        let key_b = CommandKey("bbb".into());

        tracker.record_hit(&key_a, &["echo".into()], &cwd, &[]).await;
        assert_eq!(tracker.len().await, 1);

        tracker.record_hit(&key_b, &["ls".into()], &cwd, &[]).await;
        assert_eq!(tracker.len().await, 2);

        // Same key again doesn't increase count of distinct keys
        tracker.record_hit(&key_a, &["echo".into()], &cwd, &[]).await;
        assert_eq!(tracker.len().await, 2);
    }

    #[tokio::test]
    async fn drain_below_threshold_returns_empty() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");
        let key = CommandKey("below-threshold".into());

        // Exactly threshold-1 hits
        for _ in 0..(SPECULATION_THRESHOLD - 1) {
            tracker.record_hit(&key, &["cmd".into()], &cwd, &[]).await;
        }
        assert!(tracker.drain_candidates().await.is_empty());
    }

    #[tokio::test]
    async fn drain_returns_highest_frequency_first() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");

        let key_low = CommandKey("low-freq".into());
        let key_high = CommandKey("high-freq".into());

        // Low gets exactly threshold hits
        for _ in 0..SPECULATION_THRESHOLD {
            tracker.record_hit(&key_low, &["low".into()], &cwd, &[]).await;
        }
        // High gets threshold + 2
        for _ in 0..(SPECULATION_THRESHOLD + 2) {
            tracker.record_hit(&key_high, &["high".into()], &cwd, &[]).await;
        }

        let candidates = tracker.drain_candidates().await;
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].key, key_high, "highest frequency MUST come first");
        assert_eq!(candidates[1].key, key_low);
    }

    #[tokio::test]
    async fn drain_preserves_request_details() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/opt/project");
        let key = CommandKey("detail-test".into());
        let argv = vec!["cargo".into(), "test".into()];
        let env = vec![("RUST_LOG".into(), "debug".into())];

        for _ in 0..SPECULATION_THRESHOLD {
            tracker.record_hit(&key, &argv, &cwd, &env).await;
        }

        let candidates = tracker.drain_candidates().await;
        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];
        assert_eq!(c.argv, argv);
        assert_eq!(c.cwd, cwd);
        assert_eq!(c.env, env);
    }

    #[tokio::test]
    async fn drain_does_not_return_candidates_still_below_threshold() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");
        let key_ok = CommandKey("ok".into());
        let key_low = CommandKey("low".into());

        for _ in 0..SPECULATION_THRESHOLD {
            tracker.record_hit(&key_ok, &["ok".into()], &cwd, &[]).await;
        }
        for _ in 0..(SPECULATION_THRESHOLD - 1) {
            tracker.record_hit(&key_low, &["low".into()], &cwd, &[]).await;
        }

        let candidates = tracker.drain_candidates().await;
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].key, key_ok);
    }

    #[tokio::test]
    async fn tracker_new_is_empty() {
        let tracker = SpeculationTracker::new();
        assert!(tracker.is_empty().await);
        assert_eq!(tracker.len().await, 0);
    }

    #[test]
    fn speculate_execute_multiple_args() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "echo $1 $2".into(),
            "sh".into(),
            "a".into(),
            "b".into(),
        ];
        let cwd = std::path::PathBuf::from("/tmp");
        let result = speculate_execute(&argv, &cwd, &[]).expect("execute");
        let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
        assert_eq!(stdout, "a b");
    }

    #[tokio::test]
    async fn drain_empty_tracker_returns_empty() {
        let tracker = SpeculationTracker::new();
        assert!(tracker.drain_candidates().await.is_empty());
    }

    #[tokio::test]
    async fn drain_twice_returns_only_new_candidates() {
        let tracker = SpeculationTracker::new();
        let cwd = std::path::PathBuf::from("/tmp");
        let key = CommandKey("drain-twice".into());

        for _ in 0..SPECULATION_THRESHOLD {
            tracker.record_hit(&key, &["cmd".into()], &cwd, &[]).await;
        }

        let first = tracker.drain_candidates().await;
        assert_eq!(first.len(), 1);

        // After drain, counter is reset — next drain should be empty
        let second = tracker.drain_candidates().await;
        assert!(second.is_empty());
    }

    // -----------------------------------------------------------------------
    // SpeculativeCoalescer (FR-008 in-flight request coalesce)
    // -----------------------------------------------------------------------

    /// Helper: a fresh `SpeculativeCoalescer` with a 50ms test window.
    fn test_coalescer() -> SpeculativeCoalescer {
        SpeculativeCoalescer::new(Duration::from_millis(50))
    }

    /// Helper: a successful `exec` closure that returns a fixed `CachedResult`
    /// after a short sleep, incrementing a shared counter so tests can verify
    /// the executor ran exactly N times.
    fn exec_ok(
        counter: Arc<AtomicU32>,
        payload: &'static str,
    ) -> impl FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<CachedResult>> + Send + 'static>>
    {
        move || {
            let counter = Arc::clone(&counter);
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                // Tiny sleep so concurrent callers actually coalesce rather
                // than each finishing before the next registers.
                tokio::time::sleep(Duration::from_millis(5)).await;
                Ok(CachedResult {
                    exit_code: 0,
                    stdout: payload.as_bytes().to_vec(),
                    stderr: Vec::new(),
                })
            })
        }
    }

    /// Helper: an `exec` closure that returns `Err`.
    fn exec_err()
    -> impl FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<CachedResult>> + Send + 'static>>
    {
        || {
            Box::pin(async move { Err(anyhow!("intentional exec failure")) })
        }
    }

    #[test]
    fn coalescer_default_window_is_one_hundred_ms() {
        let c = SpeculativeCoalescer::default();
        assert_eq!(c.window(), SPECULATIVE_COALESCE_WINDOW);
        assert_eq!(c.window(), Duration::from_millis(100));
    }

    #[test]
    fn coalescer_new_uses_configured_window() {
        let c = SpeculativeCoalescer::new(Duration::from_millis(250));
        assert_eq!(c.window(), Duration::from_millis(250));
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_within_window_runs_executor_once_and_shares_result() {
        let coalescer = test_coalescer();
        let key = CommandKey("within-window".into());
        let counter = Arc::new(AtomicU32::new(0));

        // All three callers reference the same `SpeculativeCoalescer` via
        // `&self` — `coalesce` does NOT consume the receiver.
        let (r1, r2, r3) = tokio::join!(
            coalescer.coalesce(
                &key,
                exec_ok(Arc::clone(&counter), "shared-payload"),
            ),
            coalescer.coalesce(
                &key,
                exec_ok(Arc::clone(&counter), "different-payload"),
            ),
            coalescer.coalesce(
                &key,
                exec_ok(Arc::clone(&counter), "another-payload"),
            ),
        );

        let r1 = r1.expect("r1 ok");
        let r2 = r2.expect("r2 ok");
        let r3 = r3.expect("r3 ok");

        // All three receive the leader's result (the first caller's payload).
        assert_eq!(r1.stdout, b"shared-payload");
        assert_eq!(r2.stdout, b"shared-payload");
        assert_eq!(r3.stdout, b"shared-payload");

        // Executor ran exactly once (the leader's closure).
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "executor MUST run exactly once for a coalesced cycle"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_after_window_starts_fresh_cycle() {
        // Use a window shorter than the test sleep so the second call lands
        // after the first cycle has drained.
        let coalescer = SpeculativeCoalescer::new(Duration::from_millis(10));
        let key = CommandKey("fresh-cycle".into());
        let counter = Arc::new(AtomicU32::new(0));

        // First cycle — runs once.
        let cnt1 = Arc::clone(&counter);
        let r1 = coalescer
            .coalesce(&key, exec_ok(Arc::clone(&cnt1), "cycle-1"))
            .await
            .expect("r1 ok");

        // Yield to give the spawned executor task a chance to drain.
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Second cycle — must run a fresh executor.
        let cnt2 = Arc::clone(&counter);
        let r2 = coalescer
            .coalesce(&key, exec_ok(Arc::clone(&cnt2), "cycle-2"))
            .await
            .expect("r2 ok");

        assert_eq!(r1.stdout, b"cycle-1");
        assert_eq!(r2.stdout, b"cycle-2");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "executions separated by more than the window MUST run twice"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_different_keys_are_independent() {
        let coalescer = test_coalescer();
        let counter = Arc::new(AtomicU32::new(0));

        let key_a = CommandKey("key-a".into());
        let key_b = CommandKey("key-b".into());

        let cnt = Arc::clone(&counter);
        let r_a = coalescer
            .coalesce(&key_a, exec_ok(Arc::clone(&cnt), "payload-a"))
            .await
            .expect("r_a");
        let cnt = Arc::clone(&counter);
        let r_b = coalescer
            .coalesce(&key_b, exec_ok(Arc::clone(&cnt), "payload-b"))
            .await
            .expect("r_b");

        assert_eq!(r_a.stdout, b"payload-a");
        assert_eq!(r_b.stdout, b"payload-b");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "different keys MUST run separate executions"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_propagates_executor_error_to_all_callers() {
        let coalescer = test_coalescer();
        let key = CommandKey("err-shared".into());

        let (r1, r2) = tokio::join!(
            coalescer.coalesce(&key, exec_err()),
            coalescer.coalesce(&key, exec_err()),
        );

        let e1 = r1.expect_err("r1 must be Err");
        let e2 = r2.expect_err("r2 must be Err");
        assert!(
            e1.to_string().contains("intentional exec failure"),
            "r1 MUST surface exec error: {e1}"
        );
        assert!(
            e2.to_string().contains("intentional exec failure"),
            "r2 MUST surface exec error: {e2}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_pending_len_zero_after_cycle() {
        let coalescer = test_coalescer();
        let key = CommandKey("lifecycle".into());

        assert_eq!(
            coalescer.pending_len().await,
            0,
            "fresh coalescer MUST have no pending"
        );

        let cnt = Arc::new(AtomicU32::new(0));
        let r = coalescer
            .coalesce(&key, exec_ok(cnt, "lifecycle"))
            .await
            .expect("ok");
        assert_eq!(r.stdout, b"lifecycle");

        // After the cycle completes, the pending group MUST be drained.
        assert_eq!(
            coalescer.pending_len().await,
            0,
            "pending_len MUST return to zero once the coalesce cycle resolves"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_pending_len_reflects_active_cycle() {
        // A 1-second window: the cycle stays pending long enough to inspect.
        let coalescer = SpeculativeCoalescer::new(Duration::from_secs(1));
        let key = CommandKey("long-window".into());

        // Manually drive a coalesce call but DO NOT await its completion —
        // hold the receiver so we can inspect `pending_len` while the cycle
        // is in-flight.
        let counter = Arc::new(AtomicU32::new(0));
        let cnt = Arc::clone(&counter);
        let fut = coalescer.coalesce(&key, exec_ok(cnt, "long"));

        // Yield once so the leader registers + the spawned task is created.
        tokio::task::yield_now().await;

        // The cycle is still pending (window hasn't elapsed under start_paused).
        assert_eq!(
            coalescer.pending_len().await,
            1,
            "pending_len MUST show 1 active group while the window is still running"
        );

        // Now advance virtual time past the window so the executor runs.
        tokio::time::sleep(Duration::from_secs(2)).await;

        // And consume the future so the spawned task completes.
        let r = fut.await.expect("ok");
        assert_eq!(r.stdout, b"long");

        assert_eq!(
            coalescer.pending_len().await,
            0,
            "pending_len MUST drop to zero after the cycle resolves"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesce_concurrent_stress_thread_safety() {
        // Hammer the coalescer from many concurrent tasks sharing the same
        // Arc'd coalescer. Verifies Arc<Mutex<…>> + oneshot broadcasting
        // remains correct under contention.
        let coalescer = Arc::new(test_coalescer());
        let key = CommandKey("stress".into());
        let counter = Arc::new(AtomicU32::new(0));

        const N: usize = 32;
        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            let c = Arc::clone(&coalescer);
            let k = key.clone();
            let cnt = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                c.coalesce(&k, exec_ok(cnt, "stress")).await
            }));
        }

        for handle in handles {
            let r = handle.await.expect("join").expect("ok");
            assert_eq!(r.stdout, b"stress", "every caller MUST see the same payload");
        }

        // With `start_paused` all 32 calls register before the window elapses,
        // so the executor runs exactly once.
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "executor MUST run once across {} concurrent callers",
            N
        );
    }
}
