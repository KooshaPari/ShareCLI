//! Native mature-recovery adversarial controls for FR-008 semantic identity.
//!
//! These exercise the real Hypervisor. They are oracle-only tests: no product
//! remediation is included. Each test asks whether a legacy cache-key mode can
//! safely serve as a generic command-result equivalence relation.

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
    let status = Command::new("git").arg("-C").arg(cwd).args(args).status()
        .expect("git must be executable");
    assert!(status.success(), "git {:?} failed", args);
}

fn hypervisor(root: &std::path::Path, mode: CacheKeyMode) -> Hypervisor {
    Hypervisor::with_options(
        HypervisorConfig {
            cache_root: root.join("cache"),
            queue_root: root.join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(300),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: mode,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
        vec![],
    )
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

    let hv = hypervisor(dir.path(), CacheKeyMode::Git);
    let request = || SpawnRequest {
        argv: vec!["cat".into(), "input.txt".into()],
        cwd: repo.clone(), env: vec![], queue_priority: QueuePriority::Normal,
    };

    fs::write(&input, b"first edit\n").expect("edit one");
    let first = hv.run(request()).await.expect("first run");
    assert_eq!(first.stdout, b"first edit\n");
    assert!(!first.from_cache);

    fs::write(&input, b"second edit\n").expect("edit two");
    let second = hv.run(request()).await.expect("second run");
    assert_eq!(second.stdout, b"second edit\n",
        "Git mode reused output although input bytes changed with the same HEAD/status shape");
    assert!(!second.from_cache);
}

#[tokio::test]
async fn fr008_time_mode_must_not_reuse_output_after_file_content_changes() {
    let dir = TempDir::new().expect("fixture");
    let work = dir.path().join("work");
    fs::create_dir(&work).expect("work dir");
    let input = work.join("input.txt");
    let hv = hypervisor(dir.path(), CacheKeyMode::Time);
    let request = || SpawnRequest {
        argv: vec!["cat".into(), "input.txt".into()],
        cwd: work.clone(), env: vec![], queue_priority: QueuePriority::Normal,
    };

    fs::write(&input, b"first\n").expect("first");
    let first = hv.run(request()).await.expect("first run");
    assert_eq!(first.stdout, b"first\n");

    fs::write(&input, b"second\n").expect("second");
    let second = hv.run(request()).await.expect("second run");
    assert_eq!(second.stdout, b"second\n",
        "Time mode reused output although cwd/argv/env stayed constant and input bytes changed");
    assert!(!second.from_cache);
}

#[tokio::test]
async fn fr008_args_mode_must_not_reuse_output_across_distinct_workspaces() {
    let dir = TempDir::new().expect("fixture");
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir(&a).expect("a");
    fs::create_dir(&b).expect("b");
    fs::write(a.join("input.txt"), b"workspace-a\n").expect("a input");
    fs::write(b.join("input.txt"), b"workspace-b\n").expect("b input");

    let hv = hypervisor(dir.path(), CacheKeyMode::Args);
    let req = |cwd: &std::path::Path| SpawnRequest {
        argv: vec!["cat".into(), "input.txt".into()],
        cwd: cwd.to_path_buf(), env: vec![], queue_priority: QueuePriority::Normal,
    };

    let first = hv.run(req(&a)).await.expect("first run");
    assert_eq!(first.stdout, b"workspace-a\n");

    let second = hv.run(req(&b)).await.expect("second run");
    assert_eq!(second.stdout, b"workspace-b\n",
        "Args mode reused output across distinct workspaces with identical argv");
    assert!(!second.from_cache);
}

#[tokio::test]
async fn fr008_git_mode_must_not_ignore_environment_that_changes_output() {
    let dir = TempDir::new().expect("fixture");
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).expect("repo dir");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "ShareCLI recovery fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(repo.join("tracked.txt"), b"stable\n").expect("tracked");
    git(&repo, &["add", "tracked.txt"]);
    git(&repo, &["commit", "-qm", "baseline"]);

    let hv = hypervisor(dir.path(), CacheKeyMode::Git);
    let req = |value: &str| SpawnRequest {
        argv: vec!["sh".into(), "-c".into(), "printf %s \"$SHARECLI_RECOVERY_VALUE\"".into()],
        cwd: repo.clone(),
        env: vec![("SHARECLI_RECOVERY_VALUE".into(), value.into())],
        queue_priority: QueuePriority::Normal,
    };

    let first = hv.run(req("first")).await.expect("first run");
    assert_eq!(first.stdout, b"first");

    let second = hv.run(req("second")).await.expect("second run");
    assert_eq!(second.stdout, b"second",
        "Git mode reused output although an execution-relevant environment value changed");
    assert!(!second.from_cache);
}
