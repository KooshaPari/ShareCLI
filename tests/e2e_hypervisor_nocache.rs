// SPDX-License-Identifier: Apache-2.0 OR MIT
//! E2E tier — Hypervisor nocache path (C07 L64 / FR-008 AC-008.10).
//! FR: FR-008
//!
//! Proves end-to-end that mutating (`nocache_args`) argv:
//! - always re-executes (side-effect counter increments per run)
//! - never sets `from_cache`
//! - serializes through Hypervisor's SlotQueue under concurrent load
//! - stays isolated from a seeded coalesce cache hit on a read-only twin

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sharecli_core::{FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision};
use tempfile::TempDir;

fn allow_hv(root: &Path) -> Hypervisor {
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    Hypervisor::with_thermal_gate(root, gate)
}

fn nocache_append_argv(counter: &Path) -> Vec<String> {
    // `--force` is a DEFAULT_NOCACHE_ARGS token; `sh -c` ignores it as $0.
    #[cfg(unix)]
    {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("printf . >> '{}'; sleep 0.04", counter.display()),
            "--force".to_string(),
        ]
    }
    #[cfg(windows)]
    {
        vec![
            "cmd".to_string(),
            "/C".to_string(),
            format!("echo.>>\"{}\" & timeout /T 1 /NOBREAK >NUL", counter.display()),
            "--force".to_string(),
        ]
    }
}

fn counter_len(path: &Path) -> usize {
    std::fs::read(path).map(|b| b.len()).unwrap_or(0)
}

/// FR-008 / AC-008.10 — identical nocache runs always re-execute (side effects).
#[tokio::test]
async fn e2e_hypervisor_nocache_double_exec_side_effect() {
    let dir = TempDir::new().expect("tempdir");
    let counter = dir.path().join("counter.txt");
    std::fs::write(&counter, b"").expect("seed counter");
    let hv = allow_hv(dir.path());
    let req = SpawnRequest {
        argv: nocache_append_argv(&counter),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    let first = hv.run(req.clone()).await.expect("first nocache");
    let second = hv.run(req).await.expect("second nocache");
    assert!(!first.from_cache, "AC-008.10: first MUST NOT be from_cache");
    assert!(!second.from_cache, "AC-008.10: second MUST NOT be from_cache");
    assert_eq!(
        counter_len(&counter),
        2,
        "AC-008.10: mutating path MUST re-execute (two side effects)"
    );
}

/// FR-008 / AC-008.10 — concurrent nocache runs serialize via Hypervisor queue.
#[tokio::test]
async fn e2e_hypervisor_nocache_concurrent_serializes() {
    let dir = TempDir::new().expect("tempdir");
    let counter = dir.path().join("counter.txt");
    std::fs::write(&counter, b"").expect("seed");
    let hv = Arc::new(allow_hv(dir.path()));
    let argv = nocache_append_argv(&counter);
    let cwd: PathBuf = dir.path().to_path_buf();

    let start = Instant::now();
    let mut joins = Vec::new();
    for _ in 0..3 {
        let hv = Arc::clone(&hv);
        let argv = argv.clone();
        let cwd = cwd.clone();
        joins.push(tokio::spawn(async move {
            hv.run(SpawnRequest { argv, cwd, env: vec![], queue_priority: QueuePriority::Normal })
                .await
        }));
    }
    let mut outcomes = Vec::new();
    for j in joins {
        outcomes.push(j.await.expect("join").expect("run"));
    }
    let elapsed = start.elapsed();

    assert!(
        outcomes.iter().all(|o| !o.from_cache),
        "AC-008.10: concurrent nocache MUST never from_cache"
    );
    assert_eq!(
        counter_len(&counter),
        3,
        "AC-008.10: three concurrent nocache runs MUST all execute"
    );
    // Each child sleeps ~40ms; max_concurrent=1 ⇒ wall clock roughly ≥ 3× sleep.
    assert!(
        elapsed >= Duration::from_millis(100),
        "AC-008.10: Hypervisor SlotQueue MUST serialize (elapsed {elapsed:?})"
    );
    assert_eq!(hv.queue().max_concurrent(), 1);
}

/// FR-008 / AC-008.10 — coalesce hit on read-only twin does not poison nocache twin.
#[tokio::test]
async fn e2e_hypervisor_nocache_isolated_from_coalesce_cache() {
    let dir = TempDir::new().expect("tempdir");
    let counter = dir.path().join("counter.txt");
    std::fs::write(&counter, b"").expect("seed");
    let hv = allow_hv(dir.path());

    #[cfg(unix)]
    let readonly =
        vec!["sh".to_string(), "-c".to_string(), format!("printf . >> '{}'", counter.display())];
    #[cfg(windows)]
    let readonly =
        vec!["cmd".to_string(), "/C".to_string(), format!("echo.>>\"{}\"", counter.display())];

    let read_req = SpawnRequest {
        argv: readonly,
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let miss = hv.run(read_req.clone()).await.expect("coalesce miss");
    assert!(!miss.from_cache);
    let hit = hv.run(read_req).await.expect("coalesce hit");
    assert!(hit.from_cache, "read-only twin MUST hit coalesce (AC-008.4)");
    assert_eq!(counter_len(&counter), 1, "coalesce MUST run once");

    let nocache_req = SpawnRequest {
        argv: nocache_append_argv(&counter),
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let mutating = hv.run(nocache_req).await.expect("nocache after coalesce");
    assert!(!mutating.from_cache, "AC-008.10: nocache MUST NOT reuse coalesce cache");
    assert_eq!(
        counter_len(&counter),
        2,
        "AC-008.10: nocache MUST still execute after coalesce seed"
    );
}
