//! Hypervisor integration tests.
//!
//! These exercise the full `Hypervisor::run` / `run_queued` paths with
//! `FakeThermalGate` (so no real hardware thermal polling happens) and a
//! real `CoalesceCache` rooted at a tempdir.  FUSE-platform tests are best-effort
//! (the guard transparently degrades to a no-op on non-Linux/macOS).

#![allow(unused_imports)]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sharecli_fleet::agent_contention::AgentContentionThresholds;
use sharecli_ipc::{CacheKeyMode, CoalesceCache, QueuePriority};
use tempfile::TempDir;

use crate::hypervisor::{queue_lane_from_argv, Hypervisor};
use crate::thermal_gate::{
    AgentAwareThermalGate, FakeThermalGate, ThermalDecision, THERMAL_MAX_RETRIES,
};
use crate::types::{HypervisorConfig, SpawnRequest};

/// Build an allow-gate hypervisor (always Green) rooted at a fresh tempdir.
fn allow_hypervisor(dir: &Path) -> Hypervisor {
    let allow: Arc<dyn crate::thermal_gate::ThermalGate> =
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    Hypervisor::with_thermal_gate(dir, allow)
}

fn echo_argv(msg: &str) -> Vec<String> {
    #[cfg(unix)]
    {
        return vec!["echo".to_string(), msg.to_string()];
    }
    #[cfg(windows)]
    {
        return vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), msg.to_string()];
    }
}

// ── Existing cache-coalescing tests ──────────────────────────────────────────

/// (a) Running a simple echo command for the first time should succeed with
///     `from_cache = false` and the expected stdout.
#[tokio::test]
async fn run_echo_fresh() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());

    let req = SpawnRequest {
        argv: echo_argv("hello"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    let outcome = hv.run(req).await.expect("run should succeed");

    assert_eq!(outcome.exit_code, 0, "echo should exit 0");
    assert!(!outcome.from_cache, "first run must not come from cache");

    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("hello"), "stdout should contain 'hello', got: {stdout:?}");
}

/// (b) A second identical run must return the cached result (`from_cache = true`)
///     with the same stdout bytes — without re-executing the process.
#[tokio::test]
async fn run_echo_coalesces_on_second_call() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());

    let req = SpawnRequest {
        argv: echo_argv("world"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    // First call — live spawn.
    let first = hv.run(req.clone()).await.expect("first run");
    assert!(!first.from_cache, "first run must not come from cache");
    assert_eq!(first.exit_code, 0);

    // Second call — must hit the cache.
    let second = hv.run(req).await.expect("second run");
    assert!(second.from_cache, "second run must come from cache");
    assert_eq!(second.stdout, first.stdout, "cached stdout must match original");
    assert_eq!(second.exit_code, first.exit_code);
}

// ── Thermal gate unit tests ─────────────────────────────────────────────────

/// Green gate: spawn must succeed immediately without any retry.
#[tokio::test]
async fn thermal_gate_green_allows_spawn() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    let req = SpawnRequest {
        argv: echo_argv("green-gate"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    let outcome = hv.run(req).await.expect("Green gate must allow spawn");
    assert_eq!(outcome.exit_code, 0);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("green-gate"));
}

/// Yellow gate: spawn must succeed (warm device does not block).
#[tokio::test]
async fn thermal_gate_yellow_allows_spawn_with_warning() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Warn));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    let req = SpawnRequest {
        argv: echo_argv("yellow-gate"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    // Yellow must not block or error.
    let outcome = hv.run(req).await.expect("Yellow gate must allow spawn");
    assert_eq!(outcome.exit_code, 0);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("yellow-gate"));
}

/// Red gate: spawn must be refused with an explicit, actionable error
/// after THERMAL_MAX_RETRIES attempts.  The error message must mention
/// "thermally throttled" so the operator can act on it.
#[tokio::test(start_paused = true)]
async fn thermal_gate_red_refuses_spawn_with_loud_error() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Refuse));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    let req = SpawnRequest {
        argv: echo_argv("red-gate"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    let result = hv.run(req).await;
    assert!(result.is_err(), "Red gate must refuse spawn");

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("thermally throttled"),
        "error must mention 'thermally throttled', got: {msg}"
    );
    assert!(
        msg.contains(&THERMAL_MAX_RETRIES.to_string()),
        "error must mention retry count, got: {msg}"
    );
}

/// A gate that transitions Green → Red → Green validates that the retry
/// loop recovers once the device cools down.
///
/// The gate starts RED for the first call, then returns Green on the second
/// call — simulating thermal recovery after one sleep-retry.
#[tokio::test(start_paused = true)]
async fn thermal_gate_recovers_after_one_red_attempt() {
    /// A gate that returns Refuse on the first check, then Allow forever.
    struct OneRedThenGreen {
        calls: AtomicU32,
    }
    impl crate::thermal_gate::ThermalGate for OneRedThenGreen {
        fn check(&self) -> ThermalDecision {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                ThermalDecision::Refuse
            } else {
                ThermalDecision::Allow
            }
        }
    }

    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(OneRedThenGreen { calls: AtomicU32::new(0) });
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    let req = SpawnRequest {
        argv: echo_argv("recover"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    // With `start_paused` tokio::time::sleep resolves immediately in tests.
    let outcome = hv.run(req).await.expect("should succeed after one RED retry");
    assert_eq!(outcome.exit_code, 0);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("recover"));
}

// ── FUSE IO-intercept tests ───────────────────────────────────────────────

/// With FUSE wired into the run() path, a spawn must still succeed
/// regardless of whether the intercept is active.
#[tokio::test]
async fn fuse_io_run_succeeds_with_or_without_intercept() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());

    let req = SpawnRequest {
        argv: echo_argv("fuse-run"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    let outcome = hv.run(req).await.expect("run must succeed with fuse-io wiring");
    assert_eq!(outcome.exit_code, 0);
    assert!(!outcome.from_cache);
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    assert!(stdout.contains("fuse-run"));
}

// ── Coalesce + queue + accessors ──────────────────────────────────────────

/// HypervisorConfig.coalesce_debounce is plumbed into CoalesceCache so every
/// Hypervisor::run coalesce path uses with_lock's debounce re-check (AC-008.6).
#[tokio::test]
async fn hypervisor_coalesce_debounce_wired_from_config() {
    let dir = TempDir::new().expect("tempdir");
    let debounce = Duration::from_millis(75);
    let hv = Hypervisor::with_options(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: CoalesceCache::DEFAULT_TTL,
            coalesce_debounce: debounce,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        vec![],
    );
    assert_eq!(
        hv.coalesce_debounce(),
        debounce,
        "AC-008.6: Hypervisor coalesce path MUST carry config debounce"
    );
}

/// Default Hypervisor constructors leave debounce disabled (opt-in via config).
#[tokio::test]
async fn hypervisor_default_debounce_is_zero() {
    let dir = TempDir::new().expect("tempdir");
    let hv = Hypervisor::new(dir.path());
    assert_eq!(hv.coalesce_debounce(), Duration::ZERO);
}

/// FR-008 / AC-008.16 — `run_queued` never serves coalesce cache.
#[tokio::test]
async fn hypervisor_run_queued_skips_coalesce_cache() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());
    let req = SpawnRequest::from_operator(
        echo_argv("queued-twice"),
        dir.path().to_path_buf(),
        vec![],
        None,
    );

    let first = hv.run_queued(req.clone(), "echo").await.expect("first run_queued");
    assert!(!first.from_cache, "AC-008.16: run_queued MUST NOT use cache on first run");

    let second = hv.run_queued(req, "echo").await.expect("second run_queued");
    assert!(!second.from_cache, "AC-008.16: run_queued MUST NOT use cache on replay");
    assert_eq!(second.exit_code, 0);
}

/// FR-011 / AC-011.4 — high agent inventory refuses spawn even when thermal Allows.
#[tokio::test(start_paused = true)]
async fn agent_aware_gate_refuses_at_contention_limit() {
    fn eight_agents() -> usize {
        8
    }
    let dir = TempDir::new().expect("tempdir");
    let inner = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let gate = Arc::new(AgentAwareThermalGate::with_agent_count(
        inner,
        AgentContentionThresholds::default(),
        eight_agents,
    ));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    let err = hv
        .run(SpawnRequest {
            argv: echo_argv("gated"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect_err("agent contention Refuse MUST err after retries");

    let msg = err.to_string();
    assert!(
        msg.contains("thermally throttled"),
        "error must mention thermally throttled, got: {msg}"
    );
}

/// queue_lane_from_argv: extracts basename from argv[0].
#[test]
fn queue_lane_from_argv_extracts_basename() {
    let argv = vec!["/usr/local/bin/cargo".into()];
    assert_eq!(queue_lane_from_argv(&argv), "cargo");
}

/// queue_lane_from_argv: returns "unknown" for empty argv.
#[test]
fn queue_lane_from_argv_empty_returns_unknown() {
    let argv: Vec<String> = vec![];
    assert_eq!(queue_lane_from_argv(&argv), "unknown");
}

/// Hypervisor accessors: coalesce_ttl, queue_max_concurrent, nocache_args.
#[test]
fn hypervisor_accessor_methods_return_config_values() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());
    assert_eq!(hv.coalesce_ttl(), CoalesceCache::DEFAULT_TTL);
    assert_eq!(hv.queue_max_concurrent(), 1);
    assert!(!hv.nocache_args().is_empty(), "default nocache_args should not be empty");
}

/// Hypervisor::set_nocache_args replaces the flag list.
#[test]
fn hypervisor_set_nocache_args_replaces_flags() {
    let dir = TempDir::new().expect("tempdir");
    let mut hv = allow_hypervisor(dir.path());
    let original_len = hv.nocache_args().len();
    hv.set_nocache_args(vec!["--force".into(), "--clean".into()]);
    assert_eq!(hv.nocache_args(), &["--force", "--clean"]);
    assert_ne!(hv.nocache_args().len(), original_len);
}

/// Spawn with a real existing cwd succeeds.
#[tokio::test]
async fn hypervisor_spawn_with_real_cwd_succeeds() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());
    let req = SpawnRequest {
        argv: echo_argv("real-cwd"),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let outcome = hv.run(req).await.expect("run with real cwd");
    assert_eq!(outcome.exit_code, 0);
}

/// Spawn with a non-existent cwd returns a spawn error (not a panic).
#[tokio::test]
async fn hypervisor_spawn_with_bad_cwd_returns_err() {
    let dir = TempDir::new().expect("tempdir");
    let hv = allow_hypervisor(dir.path());
    let bad = dir.path().join("does-not-exist-xyz");
    let req = SpawnRequest {
        argv: echo_argv("bad-cwd"),
        cwd: bad,
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let result = hv.run(req).await;
    assert!(result.is_err(), "spawn with non-existent cwd MUST return Err");
}
