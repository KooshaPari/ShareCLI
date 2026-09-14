//! FR-008 / TODO(hypervisor): speculative — pre-execute high-probability commands.
//!
//! The [`SpeculationTracker`] records command-frequency histograms in-process.
//! When a command crosses the speculation threshold the background task
//! pre-executes it during idle periods and stores the result in the
//! [`CoalesceCache`], so the next real `Hypervisor::run` call is a cache hit.
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
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use sharecli_ipc::{CachedResult, CoalesceCache, CommandKey};
use tokio::sync::Mutex;
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
}
