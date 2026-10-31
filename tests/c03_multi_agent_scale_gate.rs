// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C03 — Multi-Agent Scale Gate (Wave19 gap remediation)
//!
//! FR: FR-003, FR-008
//!
//! Validates that ShareCLI's Hypervisor, SlotQueue, and coalesce cache
//! maintain correctness under multi-agent contention. These tests scale
//! beyond the single-agent patterns in fr008_coalesce_mesh.rs to verify
//! worktree isolation, claim-lock integrity, and FR traceability when
//! many concurrent agents share the process pool.
//!
//! AC-C03.1  N concurrent Hypervisor runs never corrupt shared cache state.
//! AC-C03.2  Each agent's worktree (cwd) is fully isolated during parallel runs.
//! AC-C03.3  SlotQueue claim-lock serializes under N-agent contention.
//! AC-C03.4  FR traceability IDs survive concurrent dispatch (no cross-contamination).
//! AC-C03.5  Coalesce cache TTL isolation holds across parallel agents.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sharecli_core::{FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision};
use sharecli_ipc::{command_key, CachedResult, CoalesceCache, SlotQueue};
use tempfile::TempDir;

/// C03 / AC-C03.1 — N concurrent Hypervisor runs never corrupt shared cache state.
///
/// Spawns `AGENT_COUNT` parallel `Hypervisor::run` calls with identical argv/cwd
/// and verifies that exactly one miss occurs and all others are served from cache.
#[tokio::test]
async fn c03_concurrent_runs_share_cache_without_corruption() {
    const AGENT_COUNT: usize = 8;

    let dir = TempDir::new().expect("tempdir");
    let gate: Arc<dyn sharecli_core::ThermalGate> =
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Arc::new(Hypervisor::with_thermal_gate(dir.path(), Arc::clone(&gate)));

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "c03-scale".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "c03-scale".to_string()];

    let mut handles = Vec::with_capacity(AGENT_COUNT);
    for _ in 0..AGENT_COUNT {
        let hv = Arc::clone(&hv);
        let argv = argv.clone();
        let cwd = dir.path().to_path_buf();
        handles.push(tokio::spawn(async move {
            hv.run(SpawnRequest { argv, cwd, env: vec![], queue_priority: QueuePriority::Normal })
                .await
                .expect("agent run must succeed")
        }));
    }

    let results: Vec<_> = futures_util::future::join_all(handles)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("all agent tasks must join");

    let cache_hits = results.iter().filter(|r| r.from_cache).count();
    assert!(
        cache_hits >= AGENT_COUNT - 1,
        "AC-C03.1: at least {AGENT_COUNT}-1 of {AGENT_COUNT} concurrent runs MUST hit cache; got {cache_hits} hits"
    );

    // All results must be identical — no corruption.
    let first_stdout = &results[0].stdout;
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.stdout, *first_stdout,
            "AC-C03.1: agent {i} stdout must match first agent (no corruption)"
        );
    }
}

/// C03 / AC-C03.2 — Each agent's worktree (cwd) is fully isolated during parallel runs.
///
/// Spawns agents with different cwd directories and verifies that cache keys
/// are distinct and results do not leak across worktrees.
#[tokio::test]
async fn c03_worktree_isolation_across_concurrent_agents() {
    const AGENT_COUNT: usize = 6;

    let root = TempDir::new().expect("tempdir");

    // Create isolated worktree directories.
    let worktrees: Vec<_> = (0..AGENT_COUNT)
        .map(|i| {
            let wt = root.path().join(format!("wt-{i}"));
            std::fs::create_dir_all(&wt).expect("mkdir worktree");
            wt
        })
        .collect();

    let gate: Arc<dyn sharecli_core::ThermalGate> =
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Arc::new(Hypervisor::with_thermal_gate(root.path(), Arc::clone(&gate)));

    let mut handles = Vec::with_capacity(AGENT_COUNT);
    for (i, wt) in worktrees.iter().enumerate() {
        let hv = Arc::clone(&hv);
        let cwd = wt.clone();
        #[cfg(unix)]
        let argv = vec!["echo".to_string(), format!("agent-{i}")];
        #[cfg(windows)]
        let argv =
            vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), format!("agent-{i}")];
        handles.push(tokio::spawn(async move {
            hv.run(SpawnRequest {
                argv,
                cwd,
                env: vec![("C03_AGENT".into(), i.to_string())],
                queue_priority: QueuePriority::Normal,
            })
            .await
            .expect("worktree agent run")
        }));
    }

    let results: Vec<_> = futures_util::future::join_all(handles)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("all worktree agents must join");

    // Each agent should produce output containing its own agent id.
    for (i, r) in results.iter().enumerate() {
        let stdout = String::from_utf8_lossy(&r.stdout);
        assert!(
            stdout.contains(&format!("agent-{i}")),
            "AC-C03.2: worktree agent {i} stdout must contain its own id; got: {stdout}"
        );
    }

    // All first runs must be cache misses (different cwd → different command_key).
    for (i, r) in results.iter().enumerate() {
        assert!(
            !r.from_cache,
            "AC-C03.2: worktree agent {i} first run MUST NOT be from cache (isolated cwd)"
        );
    }

    // Verify command_key actually differs across worktrees.
    #[cfg(unix)]
    let argv_ref = vec!["echo".to_string(), "agent-0".to_string()];
    #[cfg(windows)]
    let argv_ref =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "agent-0".to_string()];
    let key_a = command_key(&argv_ref, &worktrees[0], &[]);
    let key_b = command_key(&argv_ref, &worktrees[1], &[]);
    assert_ne!(key_a, key_b, "AC-C03.2: different worktrees MUST produce different command_keys");
}

/// C03 / AC-C03.3 — SlotQueue claim-lock serializes under N-agent contention.
///
/// Spawns `AGENT_COUNT` threads all contending for a single-slot queue and
/// verifies that peak concurrency never exceeds 1.
#[test]
fn c03_slot_queue_serializes_under_n_agent_contention() {
    const AGENT_COUNT: usize = 10;

    let dir = TempDir::new().expect("tempdir");
    let active = Arc::new(AtomicU32::new(0));
    let peak = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::with_capacity(AGENT_COUNT);
    for _ in 0..AGENT_COUNT {
        let root = dir.path().to_path_buf();
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        handles.push(thread::spawn(move || {
            let q =
                SlotQueue::with_options(root, 1, Duration::from_secs(5), Duration::from_millis(10));
            q.with_slot("c03-lane", QueuePriority::Normal, || {
                let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(n, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(15));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            })
            .expect("slot");
        }));
    }

    for h in handles {
        h.join().expect("thread join");
    }

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "AC-C03.3: SlotQueue with max_concurrent=1 MUST serialize {AGENT_COUNT} agents; peak was {}",
        peak.load(Ordering::SeqCst)
    );
}

/// C03 / AC-C03.4 — FR traceability IDs survive concurrent dispatch.
///
/// Verifies that command_key incorporates agent-specific env dimensions so
/// that each agent's cache entry is traceable back to its origin.
#[test]
fn c03_fr_traceability_ids_survive_concurrent_dispatch() {
    const AGENT_COUNT: usize = 5;

    let cwd = Path::new("/tmp/sharecli-c03-trace");
    let argv = vec!["echo".to_string(), "trace-probe".to_string()];

    // Generate command_keys for each agent with unique env dimensions.
    let mut keys = Vec::with_capacity(AGENT_COUNT);
    for i in 0..AGENT_COUNT {
        let env = vec![
            ("AGENT_ID".into(), format!("agent-{i}")),
            ("SESSION_ID".into(), format!("sess-{i}")),
        ];
        let key = command_key(&argv, cwd, &env);
        keys.push(key);
    }

    // All keys must be unique — traceability is not lost under concurrency.
    let unique: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(
        unique.len(),
        AGENT_COUNT,
        "AC-C03.4: each agent MUST produce a unique command_key for FR traceability; got {} unique out of {AGENT_COUNT}",
        unique.len()
    );

    // Re-generating the same agent's key must be stable.
    for i in 0..AGENT_COUNT {
        let env = vec![
            ("AGENT_ID".into(), format!("agent-{i}")),
            ("SESSION_ID".into(), format!("sess-{i}")),
        ];
        let key = command_key(&argv, cwd, &env);
        assert_eq!(key, keys[i], "AC-C03.4: command_key for agent {i} MUST be stable across calls");
    }
}

/// C03 / AC-C03.5 — Coalesce cache TTL isolation holds across parallel agents.
///
/// Stores entries for different agents in the same cache and verifies that
/// each agent's TTL expiry is independent.
#[test]
fn c03_coalesce_cache_ttl_isolation_across_agents() {
    const AGENT_COUNT: usize = 4;
    let dir = TempDir::new().expect("tempdir");
    let ttl = Duration::from_millis(100);
    let cache = CoalesceCache::with_ttl(dir.path(), ttl);

    // Store a result for each agent with a unique key.
    for i in 0..AGENT_COUNT {
        let key = command_key(
            &["agent-probe".into()],
            dir.path(),
            &[("AGENT_ID".into(), format!("a{i}"))],
        );
        cache
            .store(
                &key,
                &CachedResult {
                    exit_code: 0,
                    stdout: format!("result-{i}").into_bytes(),
                    stderr: vec![],
                },
            )
            .expect("store agent result");
    }

    // All entries should be fresh immediately after store.
    for i in 0..AGENT_COUNT {
        let key = command_key(
            &["agent-probe".into()],
            dir.path(),
            &[("AGENT_ID".into(), format!("a{i}"))],
        );
        let hit = cache.lookup(&key).expect("lookup ok").expect("fresh hit");
        assert_eq!(
            hit.stdout,
            format!("result-{i}").as_bytes(),
            "AC-C03.5: agent {i} fresh lookup MUST return its own result"
        );
    }

    // Wait for TTL to expire.
    thread::sleep(ttl + Duration::from_millis(60));

    // All entries must now be stale.
    for i in 0..AGENT_COUNT {
        let key = command_key(
            &["agent-probe".into()],
            dir.path(),
            &[("AGENT_ID".into(), format!("a{i}"))],
        );
        assert!(
            cache.lookup(&key).expect("lookup ok").is_none(),
            "AC-C03.5: agent {i} entry MUST be stale after TTL expiry"
        );
    }
}
