//! FR-008 — Speculative Coalesce / Debounce / Queue
//! FR: FR-008
//!
//! AC-008.1 command_key stability
//! AC-008.2 CoalesceCache with_lock runs once
//! AC-008.3 thermal Refuse gates before coalesce/spawn
//! AC-008.4 second identical Hypervisor run hits cache
//! AC-008.5 CoalesceCache TTL treats stale entries as miss
//! AC-008.6 debounce window waits/shares before re-run
//! AC-008.7 nocache_args mutating flags bypass coalesce
//! AC-008.8 SlotQueue serializes max_concurrent=1
//! AC-008.9 Hypervisor routes nocache argv through queue (not cache)
//! AC-008.13 command_key cwd/env dimensions + Hypervisor cache isolation
//! AC-008.14 SlotQueue Critical dequeues before Normal under contention (Hypervisor nocache)
//! (Mesh membership ACs live under FR-010.)

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use sharecli_core::{
    FakeThermalGate, Hypervisor, HypervisorConfig, QueuePriority, SpawnRequest, ThermalDecision,
    THERMAL_MAX_RETRIES,
};
use sharecli_ipc::{
    command_key, has_nocache_arg, should_bypass_coalesce, CacheKeyMode, CachedResult,
    CoalesceCache, SlotQueue, DEFAULT_NOCACHE_ARGS,
};
use tempfile::TempDir;

/// FR-008 / AC-008.1 — identical argv/cwd/env → same key; different argv → different.
#[test]
fn fr008_command_key_stable() {
    let cwd = Path::new("/tmp");
    let a = command_key(&["echo".into(), "x".into()], cwd, &[]);
    let b = command_key(&["echo".into(), "x".into()], cwd, &[]);
    let c = command_key(&["echo".into(), "y".into()], cwd, &[]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// FR-008 / AC-008.13 — `command_key` incorporates cwd and env_subset dimensions.
#[test]
fn fr008_command_key_cwd_env_dimensions() {
    let argv = vec!["echo".into(), "mesh".into()];
    let cwd_a = Path::new("/tmp/sharecli-a");
    let cwd_b = Path::new("/tmp/sharecli-b");
    let env_v1 = vec![("TOOL".into(), "1".into())];
    let env_v2 = vec![("TOOL".into(), "2".into())];
    let env_perm = vec![("B".into(), "2".into()), ("A".into(), "1".into())];
    let env_sorted = vec![("A".into(), "1".into()), ("B".into(), "2".into())];

    let base = command_key(&argv, cwd_a, &env_v1);
    assert_eq!(base, command_key(&argv, cwd_a, &env_v1), "stable cwd+env");
    assert_ne!(
        command_key(&argv, cwd_b, &env_v1),
        base,
        "different cwd MUST change command_key (AC-008.13)"
    );
    assert_ne!(
        command_key(&argv, cwd_a, &env_v2),
        base,
        "different env value MUST change command_key (AC-008.13)"
    );
    assert_eq!(
        command_key(&argv, cwd_a, &env_perm),
        command_key(&argv, cwd_a, &env_sorted),
        "env key order MUST NOT affect command_key (AC-008.13)"
    );
}

/// FR-008 / AC-008.13 — Hypervisor coalesce cache isolates cwd/env, not argv alone.
#[tokio::test]
async fn fr008_hypervisor_cache_respects_cwd_and_env() {
    let root = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(root.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "cwd-env".to_string()];
    #[cfg(windows)]
    let argv = vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "cwd-env".to_string()];

    let cwd_a = root.path().join("a");
    let cwd_b = root.path().join("b");
    std::fs::create_dir_all(&cwd_a).expect("mkdir a");
    std::fs::create_dir_all(&cwd_b).expect("mkdir b");

    let req_a = SpawnRequest {
        argv: argv.clone(),
        cwd: cwd_a.clone(),
        env: vec![("SHARECLI_PROBE".into(), "1".into())],
        queue_priority: QueuePriority::Normal,
    };
    let first = hv.run(req_a.clone()).await.expect("first run");
    assert!(!first.from_cache);
    let replay = hv.run(req_a).await.expect("replay same cwd+env");
    assert!(replay.from_cache, "identical cwd+env MUST hit cache (AC-008.13)");

    let req_b = SpawnRequest {
        argv,
        cwd: cwd_b,
        env: vec![("SHARECLI_PROBE".into(), "2".into())],
        queue_priority: QueuePriority::Normal,
    };
    let isolated = hv.run(req_b).await.expect("different cwd+env");
    assert!(
        !isolated.from_cache,
        "different cwd/env MUST NOT reuse argv-only cache entry (AC-008.13)"
    );
}

/// FR-008 / AC-008.2 — with_lock executes the miss path once per key.
#[test]
fn fr008_with_lock_runs_once() {
    let dir = TempDir::new().expect("tempdir");
    let cache = CoalesceCache::new(dir.path());
    let key = command_key(&["true".into()], dir.path(), &[]);
    let hits = AtomicU32::new(0);

    let first = cache
        .with_lock(&key, || {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(CachedResult { exit_code: 0, stdout: b"once".to_vec(), stderr: vec![] })
        })
        .expect("first lock");
    let second = cache
        .with_lock(&key, || {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(CachedResult { exit_code: 1, stdout: b"again".to_vec(), stderr: vec![] })
        })
        .expect("second lock");

    assert_eq!(hits.load(Ordering::SeqCst), 1, "miss path MUST run once");
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(second.exit_code, 0);
}

/// FR-008 / AC-008.3 — Refuse thermal gate fails loudly before speculative work.
#[tokio::test(start_paused = true)]
async fn fr008_thermal_gate_before_coalesce() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Refuse));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "should-not-run".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "should-not-run".to_string()];

    let err = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect_err("Refuse MUST err after retries");

    let msg = err.to_string();
    assert!(
        msg.contains("thermally throttled"),
        "error must mention thermally throttled, got {msg}; max_retries={THERMAL_MAX_RETRIES}"
    );
}

/// FR-008 / AC-008.4 — second identical Allow run is served from coalesce cache.
#[tokio::test]
async fn fr008_second_run_from_cache() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "coalesce".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "coalesce".to_string()];

    let req = SpawnRequest {
        argv,
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let first = hv.run(req.clone()).await.expect("first");
    assert!(!first.from_cache);
    let second = hv.run(req).await.expect("second");
    assert!(second.from_cache, "mesh coalesce MUST hit cache on replay");
    assert_eq!(first.stdout, second.stdout);
}

/// FR-008 / AC-008.5 — lookup treats entries older than TTL as a miss.
/// GATED: Uses `thread::sleep`; skipped on Windows to avoid CI timeout hangs.
#[cfg(not(target_os = "windows"))]
#[test]
fn fr008_ttl_stale_entry_is_miss() {
    let dir = TempDir::new().expect("tempdir");
    let ttl = Duration::from_millis(80);
    let cache = CoalesceCache::with_ttl(dir.path(), ttl);
    let key = command_key(&["ttl-probe".into()], dir.path(), &[]);

    cache
        .store(&key, &CachedResult { exit_code: 0, stdout: b"fresh".to_vec(), stderr: vec![] })
        .expect("store");

    let hit = cache.lookup(&key).expect("lookup").expect("fresh hit");
    assert_eq!(hit.stdout, b"fresh");

    thread::sleep(ttl + Duration::from_millis(40));

    assert!(
        cache.lookup(&key).expect("stale lookup").is_none(),
        "entry older than TTL MUST be treated as miss (AC-008.5)"
    );

    let hits = AtomicU32::new(0);
    let rerun = cache
        .with_lock(&key, || {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(CachedResult { exit_code: 0, stdout: b"rerun".to_vec(), stderr: vec![] })
        })
        .expect("with_lock after TTL");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "stale miss MUST re-run");
    assert_eq!(rerun.stdout, b"rerun");
}

/// FR-008 / AC-008.6 — debounce waits then shares a result completed in-window.
/// GATED: Uses `thread::sleep`; skipped on Windows to avoid CI timeout hangs.
#[cfg(not(target_os = "windows"))]
#[test]
fn fr008_debounce_waits_and_shares() {
    let dir = TempDir::new().expect("tempdir");
    let debounce = Duration::from_millis(120);
    let cache = CoalesceCache::with_options(dir.path(), Duration::from_secs(300), debounce);
    let key = command_key(&["debounce-probe".into()], dir.path(), &[]);
    let hits = Arc::new(AtomicU32::new(0));

    let cache_bg = CoalesceCache::with_options(dir.path(), Duration::from_secs(300), debounce);
    let key_bg = key.clone();
    let hits_bg = Arc::clone(&hits);
    let producer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(40));
        cache_bg
            .store(
                &key_bg,
                &CachedResult { exit_code: 0, stdout: b"shared".to_vec(), stderr: vec![] },
            )
            .expect("bg store");
        hits_bg.fetch_add(1, Ordering::SeqCst);
    });

    let result = cache
        .with_lock(&key, || {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(CachedResult { exit_code: 1, stdout: b"should-not-run".to_vec(), stderr: vec![] })
        })
        .expect("debounced with_lock");

    producer.join().expect("producer join");

    assert_eq!(result.stdout, b"shared", "debounce MUST share in-window result");
    assert_eq!(result.exit_code, 0, "shared result MUST come from producer, not miss path");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "miss path MUST NOT run when debounce shares a recent store (AC-008.6)"
    );
}

/// FR-008 / AC-008.6 — Hypervisor::run coalesce path debounces before spawn.
/// GATED: Uses `thread::sleep`; skipped on Windows to avoid CI timeout hangs.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn fr008_hypervisor_debounce_waits_and_shares() {
    let dir = TempDir::new().expect("tempdir");
    let debounce = Duration::from_millis(120);
    let cache_root = dir.path().join("cache");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_options(
        HypervisorConfig {
            cache_root: cache_root.clone(),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: debounce,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
            error_ttl: Duration::ZERO,
        },
        gate,
        vec![],
    );

    let argv = vec!["debounce-hv-probe".to_string()];
    let cwd = dir.path().to_path_buf();
    let key = command_key(&argv, &cwd, &[]);

    let cache_bg = CoalesceCache::with_options(&cache_root, Duration::from_secs(300), debounce);
    let key_bg = key.clone();
    let producer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(40));
        cache_bg
            .store(
                &key_bg,
                &CachedResult { exit_code: 0, stdout: b"shared-hv".to_vec(), stderr: vec![] },
            )
            .expect("bg store");
    });

    let outcome = hv
        .run(SpawnRequest { argv, cwd, env: vec![], queue_priority: QueuePriority::Normal })
        .await
        .expect("debounced hypervisor run");

    producer.join().expect("producer join");

    assert_eq!(
        outcome.stdout, b"shared-hv",
        "Hypervisor debounce MUST share in-window result (AC-008.6)"
    );
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.from_cache, "debounce share MUST surface as from_cache on Hypervisor::run");
}

/// FR-008 / AC-008.7 — mutating nocache_args bypass coalesce detection.
#[test]
fn fr008_nocache_args_bypass_coalesce() {
    assert!(
        should_bypass_coalesce(&["ruff", "check", "--fix", "."], DEFAULT_NOCACHE_ARGS),
        "AC-008.7: --fix MUST bypass coalesce"
    );
    assert!(
        has_nocache_arg(&["eslint", "--fix", "src"], DEFAULT_NOCACHE_ARGS),
        "AC-008.7: --fix exact match"
    );
    assert!(
        has_nocache_arg(&["tool", "--force"], DEFAULT_NOCACHE_ARGS),
        "AC-008.7: --force is a default mutating flag"
    );
    assert!(
        has_nocache_arg(&["tool", "--write"], DEFAULT_NOCACHE_ARGS),
        "AC-008.7: --write is a default mutating flag"
    );
    assert!(
        !should_bypass_coalesce(&["ruff", "check", "."], DEFAULT_NOCACHE_ARGS),
        "read-only check MUST remain coalesce-eligible"
    );
}

/// FR-008 / AC-008.8 — SlotQueue with max_concurrent=1 serializes work.
/// GATED: Uses `thread::sleep`; skipped on Windows to avoid CI timeout hangs.
#[cfg(not(target_os = "windows"))]
#[test]
fn fr008_slot_queue_serializes() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    let dir = TempDir::new().expect("tempdir");
    let active = Arc::new(AtomicU32::new(0));
    let peak = Arc::new(AtomicU32::new(0));
    let mut handles = vec![];
    for _ in 0..3 {
        let root = dir.path().to_path_buf();
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        handles.push(thread::spawn(move || {
            let q =
                SlotQueue::with_options(root, 1, Duration::from_secs(5), Duration::from_millis(15));
            q.with_slot("lane", QueuePriority::Normal, || {
                let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(n, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(35));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            })
            .expect("slot");
        }));
    }
    for h in handles {
        h.join().expect("join");
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1, "AC-008.8: max_concurrent=1 MUST serialize");
}

/// FR-008 / AC-008.9 — Hypervisor nocache path never serves from_cache.
#[tokio::test]
async fn fr008_hypervisor_nocache_routes_to_queue() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "--force".to_string(), "nocache".to_string()];
    #[cfg(windows)]
    let argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo".to_string(),
        "--force".to_string(),
        "nocache".to_string(),
    ];

    // echo ignores unknown flags on unix; we care about routing, not exit.
    let req = SpawnRequest {
        argv,
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };
    let first = hv.run(req.clone()).await.expect("first nocache run");
    assert!(!first.from_cache, "AC-008.9: nocache MUST NOT use coalesce cache");
    let second = hv.run(req).await.expect("second nocache run");
    assert!(!second.from_cache, "AC-008.9: second identical mutating run MUST still bypass cache");
    // Queue API is exposed for Hypervisor callers.
    assert_eq!(hv.queue().max_concurrent(), 1);
    assert!(hv.nocache_args().iter().any(|f| f == "--force"));
}

/// FR-008 / AC-008.14 — `QueuePriority::Critical` acquires before `Normal` under contention.
/// GATED: Uses `thread::sleep`; skipped on Windows to avoid CI timeout hangs.
#[cfg(not(target_os = "windows"))]
#[test]
fn fr008_slot_queue_critical_before_normal() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    let dir = TempDir::new().expect("tempdir");
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
    let holder_ready = Arc::new(AtomicBool::new(false));
    let release_holder = Arc::new(AtomicBool::new(false));

    let holder_order = Arc::clone(&order);
    let holder_ready_flag = Arc::clone(&holder_ready);
    let holder_release = Arc::clone(&release_holder);
    let holder_root = dir.path().to_path_buf();
    let holder = thread::spawn(move || {
        let q = SlotQueue::with_options(
            holder_root,
            1,
            Duration::from_secs(5),
            Duration::from_millis(5),
        );
        q.with_slot("lane", QueuePriority::Normal, || {
            holder_order.lock().expect("lock").push("holder_start");
            holder_ready_flag.store(true, Ordering::SeqCst);
            while !holder_release.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
            }
            holder_order.lock().expect("lock").push("holder_end");
            Ok(())
        })
        .expect("holder slot");
    });

    while !holder_ready.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(2));
    }

    let critical_order = Arc::clone(&order);
    let critical_root = dir.path().to_path_buf();
    let critical = thread::spawn(move || {
        let q = SlotQueue::with_options(
            critical_root,
            1,
            Duration::from_secs(5),
            Duration::from_millis(5),
        );
        q.with_slot("lane", QueuePriority::Critical, || {
            critical_order.lock().expect("lock").push("critical");
            Ok(())
        })
        .expect("critical slot");
    });

    let late_normal_order = Arc::clone(&order);
    let late_normal_root = dir.path().to_path_buf();
    let late_normal = thread::spawn(move || {
        let q = SlotQueue::with_options(
            late_normal_root,
            1,
            Duration::from_secs(5),
            Duration::from_millis(5),
        );
        q.with_slot("lane", QueuePriority::Normal, || {
            late_normal_order.lock().expect("lock").push("normal_late");
            Ok(())
        })
        .expect("late normal slot");
    });

    // Ensure both waiters are registered before the holder releases the slot.
    let waiting = dir.path().join("lane.waiting");
    let wait_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < wait_deadline {
        let count = fs::read_dir(&waiting).map(|rd| rd.count()).unwrap_or(0);
        if count >= 2 {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }

    thread::sleep(Duration::from_millis(20));
    release_holder.store(true, Ordering::SeqCst);
    holder.join().expect("holder join");
    critical.join().expect("critical join");
    late_normal.join().expect("late normal join");

    let seq = order.lock().expect("lock").clone();
    assert_eq!(
        seq,
        vec!["holder_start", "holder_end", "critical", "normal_late"],
        "AC-008.14: Critical MUST dequeue before Normal under contention"
    );
}

/// FR-008 / AC-008.14 — Hypervisor nocache lane honors `SpawnRequest::queue_priority`.
#[tokio::test]
async fn fr008_hypervisor_nocache_critical_before_normal() {
    let dir = TempDir::new().expect("tempdir");
    let order_path = dir.path().join("order.log");
    std::fs::write(&order_path, b"").expect("seed order log");

    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Arc::new(Hypervisor::with_thermal_gate(dir.path(), gate));

    #[cfg(unix)]
    let hold_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "printf 'holder_start\n' >> '{}'; sleep 0.15; printf 'holder_end\n' >> '{}'",
            order_path.display(),
            order_path.display()
        ),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let hold_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!(
            "echo holder_start>>\"{}\" & timeout /T 2 /NOBREAK >NUL & echo holder_end>>\"{}\"",
            order_path.display(),
            order_path.display()
        ),
        "--force".to_string(),
    ];

    let order_log = order_path.display().to_string();

    #[cfg(unix)]
    let critical_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("printf 'critical\n' >> '{order_log}'"),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let critical_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!("echo critical>>\"{order_log}\""),
        "--force".to_string(),
    ];

    #[cfg(unix)]
    let late_normal_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("printf 'normal_late\n' >> '{order_log}'"),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let late_normal_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!("echo normal_late>>\"{order_log}\""),
        "--force".to_string(),
    ];

    let cwd = dir.path().to_path_buf();
    let holder_hv = Arc::clone(&hv);
    let holder_cwd = cwd.clone();
    let holder = tokio::spawn(async move {
        holder_hv
            .run(SpawnRequest {
                argv: hold_argv,
                cwd: holder_cwd,
                env: vec![],
                queue_priority: QueuePriority::Normal,
            })
            .await
            .expect("holder nocache run")
    });

    while !std::fs::read_to_string(&order_path).map(|s| s.contains("holder_start")).unwrap_or(false)
    {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let critical_hv = Arc::clone(&hv);
    let critical_cwd = cwd.clone();
    let critical = tokio::spawn(async move {
        critical_hv
            .run(SpawnRequest {
                argv: critical_argv,
                cwd: critical_cwd,
                env: vec![],
                queue_priority: QueuePriority::Critical,
            })
            .await
            .expect("critical nocache run")
    });

    let late_normal_hv = Arc::clone(&hv);
    let late_normal = tokio::spawn(async move {
        late_normal_hv
            .run(SpawnRequest {
                argv: late_normal_argv,
                cwd,
                env: vec![],
                queue_priority: QueuePriority::Normal,
            })
            .await
            .expect("late normal nocache run")
    });

    holder.await.expect("holder join");
    critical.await.expect("critical join");
    late_normal.await.expect("late normal join");

    let order = std::fs::read_to_string(&order_path).expect("read order log");
    let lines: Vec<&str> = order.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines,
        vec!["holder_start", "holder_end", "critical", "normal_late"],
        "AC-008.14: Hypervisor nocache Critical MUST run before queued Normal"
    );
}

/// FR-008 / AC-008.19 — Args cache_key_mode shares cache across cwd.
#[tokio::test]
async fn fr008_hypervisor_args_cache_key_mode_ignores_cwd() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let cache_root = dir.path().join("cache");
    let hv = Hypervisor::with_options(
        HypervisorConfig {
            cache_root,
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Args,
            semantic: false,
            error_ttl: Duration::ZERO,
        },
        gate,
        vec![],
    );

    let sub_a = dir.path().join("a");
    let sub_b = dir.path().join("b");
    fs::create_dir_all(&sub_a).expect("mkdir a");
    fs::create_dir_all(&sub_b).expect("mkdir b");

    let argv = vec!["/bin/echo".into(), "args-mode-probe".into()];
    let first = hv.run(SpawnRequest::new(argv.clone(), sub_a, vec![])).await.expect("first run");
    assert!(!first.from_cache);

    let second = hv.run(SpawnRequest::new(argv, sub_b, vec![])).await.expect("second run");
    assert!(second.from_cache, "AC-008.19: Args mode MUST coalesce across different cwd");
}

/// FR-008 / AC-008.20 — semantic normalization applied before Hypervisor cache hash.
#[cfg(unix)]
#[tokio::test]
async fn fr008_hypervisor_semantic_coalesces_repeated_lint_dot() {
    let dir = TempDir::new().expect("tempdir");
    let fake_ruff = dir.path().join("ruff");
    std::os::unix::fs::symlink("/bin/echo", &fake_ruff).expect("symlink ruff→echo");

    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_options(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Time,
            semantic: true,
            error_ttl: Duration::ZERO,
        },
        gate,
        vec![],
    );

    let argv = vec![fake_ruff.to_string_lossy().into(), "check".into(), ".".into()];
    let cwd = dir.path().to_path_buf();

    let first = hv
        .run(SpawnRequest::new(argv.clone(), cwd.clone(), vec![]))
        .await
        .expect("first semantic lint run");
    assert!(!first.from_cache);

    let second =
        hv.run(SpawnRequest::new(argv, cwd, vec![])).await.expect("second semantic lint run");
    assert!(second.from_cache, "AC-008.20: semantic lint invocations MUST coalesce on replay");
}
