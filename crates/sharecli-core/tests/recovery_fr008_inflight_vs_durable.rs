//! Architecture experiment: distinguish useful in-flight duplicate suppression
//! from unsafe durable replay under the current coupled Lock-Wait-Cache path.
//!
//! No product remediation is included.
//! Recovery rerun marker: 2026-09-30 after ontology/admission slices.

#![cfg(unix)]

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use sharecli_core::{
    FakeThermalGate, Hypervisor, HypervisorConfig, QueuePriority, SpawnRequest, ThermalDecision,
};
use sharecli_ipc::CacheKeyMode;
use tempfile::TempDir;

fn req(cwd: &std::path::Path, counter: &std::path::Path, input: &std::path::Path) -> SpawnRequest {
    let script = format!(
        "n=$(cat {counter} 2>/dev/null || echo 0); n=$((n+1)); printf '%s' "$n" > {counter}; sleep 0.25; cat {input}",
        counter = counter.display(),
        input = input.display(),
    );
    SpawnRequest {
        argv: vec!["sh".into(), "-c".into(), script],
        cwd: cwd.to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_share_is_useful_but_must_not_imply_later_replay() {
    let dir = TempDir::new().expect("fixture");
    let work = dir.path().join("work");
    fs::create_dir(&work).expect("work");
    let input = dir.path().join("input.txt");
    let counter = dir.path().join("exec-count.txt");
    fs::write(&input, b"v1\n").expect("input v1");

    let hv = Arc::new(Hypervisor::with_options(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 2,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        vec![],
    ));

    let a = {
        let hv = Arc::clone(&hv);
        let r = req(&work, &counter, &input);
        tokio::spawn(async move { hv.run(r).await.expect("concurrent A") })
    };
    tokio::time::sleep(Duration::from_millis(30)).await;
    let b = {
        let hv = Arc::clone(&hv);
        let r = req(&work, &counter, &input);
        tokio::spawn(async move { hv.run(r).await.expect("concurrent B") })
    };

    let (a, b) = tokio::join!(a, b);
    let a = a.expect("join A");
    let b = b.expect("join B");

    assert_eq!(a.stdout, b"v1\n");
    assert_eq!(b.stdout, b"v1\n");
    assert_eq!(
        fs::read_to_string(&counter).expect("counter"),
        "1",
        "two concurrent equivalent requests should execute the underlying command once"
    );

    // Now the in-flight execution is over. Change an execution-relevant input
    // without changing the Time-mode key dimensions.
    fs::write(&input, b"v2\n").expect("input v2");
    let later = hv.run(req(&work, &counter, &input)).await.expect("later replay");

    assert_eq!(
        later.stdout, b"v2\n",
        "current in-flight equivalence must not be treated as authority for later durable replay"
    );
    assert_eq!(
        fs::read_to_string(&counter).expect("counter after later"),
        "2",
        "later invocation after changed input must execute again"
    );
    assert!(!later.from_cache);
}
