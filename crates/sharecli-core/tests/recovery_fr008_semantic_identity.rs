//! Native mature-recovery adversarial control for FR-008 semantic identity.
//!
//! This lives in sharecli-core so it can execute without compiling unrelated tray targets.
//! It is an oracle-only test: no product remediation is included.

#![cfg(unix)]

use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use sharecli_core::{
    FakeThermalGate, Hypervisor, HypervisorConfig, QueuePriority, SpawnRequest, ThermalDecision,
};
use sharecli_ipc::CacheKeyMode;
use tempfile::TempDir;

fn git(cwd: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .status()
        .expect("git must be executable");
    assert!(status.success(), "git {:?} failed", args);
}

#[tokio::test]
async fn fr008_git_mode_must_not_reuse_output_after_content_changes() {
    let dir = TempDir::new().expect("fixture");
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).expect("repo dir");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "ShareCLI recovery fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);

    let input = repo.join("input.txt");
    fs::write(&input, b"base\n").expect("baseline");
    git(&repo, &["add", "input.txt"]);
    git(&repo, &["commit", "-qm", "baseline"]);

    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_options(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Git,
            semantic: false,
        },
        gate,
        vec![],
    );

    let request = || SpawnRequest {
        argv: vec!["cat".into(), "input.txt".into()],
        cwd: repo.clone(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    };

    fs::write(&input, b"first edit\n").expect("edit one");
    let first = hv.run(request()).await.expect("first run");
    assert_eq!(first.stdout, b"first edit\n");
    assert!(!first.from_cache);

    fs::write(&input, b"second edit\n").expect("edit two");
    let second = hv.run(request()).await.expect("second run");

    assert_eq!(
        second.stdout, b"second edit\n",
        "FR-008 false green: Git-mode cache reused output although input bytes changed"
    );
    assert!(
        !second.from_cache,
        "changed input bytes must invalidate the result-equivalence class"
    );
}
