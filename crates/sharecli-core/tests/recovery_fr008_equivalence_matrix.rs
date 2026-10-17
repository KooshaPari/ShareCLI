// FR: FR-008 mature-recovery equivalence safety oracle
//! Mature-recovery adversarial matrix for cache equivalence domains.
//!
//! These are oracle tests, not product remediation. They intentionally exercise
//! dimensions that current generic key modes omit.

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

fn hypervisor(dir: &TempDir, mode: CacheKeyMode) -> Hypervisor {
    Hypervisor::with_options(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
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

fn req(cwd: &std::path::Path, argv: &[&str], env: &[(&str, &str)]) -> SpawnRequest {
    SpawnRequest {
        argv: argv.iter().map(|x| (*x).to_string()).collect(),
        cwd: cwd.to_path_buf(),
        env: env.iter().map(|(k,v)| ((*k).to_string(),(*v).to_string())).collect(),
        queue_priority: QueuePriority::Normal,
    }
}

#[tokio::test]
async fn args_mode_must_not_share_across_different_workspaces() {
    let dir = TempDir::new().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("input.txt"), b"workspace-a\n").unwrap();
    fs::write(b.join("input.txt"), b"workspace-b\n").unwrap();

    let hv = hypervisor(&dir, CacheKeyMode::Args);
    let first = hv.run(req(&a, &["cat","input.txt"], &[])).await.unwrap();
    let second = hv.run(req(&b, &["cat","input.txt"], &[])).await.unwrap();

    assert_eq!(first.stdout, b"workspace-a\n");
    assert_eq!(second.stdout, b"workspace-b\n", "Args-mode false green: cwd-independent key reused another workspace's result");
    assert!(!second.from_cache);
}

#[tokio::test]
async fn git_mode_must_not_share_across_different_environment() {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path().join("repo");
    fs::create_dir(&cwd).unwrap();
    let status = Command::new("git").arg("-C").arg(&cwd).args(["init","-q"]).status().unwrap();
    assert!(status.success());

    let hv = hypervisor(&dir, CacheKeyMode::Git);
    let first = hv.run(req(&cwd, &["sh","-c","printf %s \"$RECOVERY_VALUE\""], &[("RECOVERY_VALUE","one")])).await.unwrap();
    let second = hv.run(req(&cwd, &["sh","-c","printf %s \"$RECOVERY_VALUE\""], &[("RECOVERY_VALUE","two")])).await.unwrap();

    assert_eq!(first.stdout, b"one");
    assert_eq!(second.stdout, b"two", "Git-mode false green: environment change reused prior result");
    assert!(!second.from_cache);
}

#[tokio::test]
async fn time_mode_must_not_replay_nondeterministic_external_state() {
    let dir = TempDir::new().unwrap();
    let cwd = dir.path().join("work");
    fs::create_dir(&cwd).unwrap();
    let external = dir.path().join("external.txt");
    fs::write(&external, b"outside-one\n").unwrap();

    let hv = hypervisor(&dir, CacheKeyMode::Time);
    let path = external.to_string_lossy().to_string();
    let first = hv.run(req(&cwd, &["cat",&path], &[])).await.unwrap();
    fs::write(&external, b"outside-two\n").unwrap();
    let second = hv.run(req(&cwd, &["cat",&path], &[])).await.unwrap();

    assert_eq!(first.stdout, b"outside-one\n");
    assert_eq!(second.stdout, b"outside-two\n", "Time-mode false green: external input change reused prior result");
    assert!(!second.from_cache);
}
