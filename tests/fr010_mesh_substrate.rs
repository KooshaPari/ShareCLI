// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-010 — Agent Mesh / Shared Substrate
//! FR: FR-010
//!
//! AC-010.1 default subject prefix
//! AC-010.2 subject_for shape
//! AC-010.3 DeviceRecord round-trip; register without NATS fails loudly
//! AC-010.4 Maildir enqueue → claim → ack lifecycle
//! AC-010.5 Maildir priority ordering (lower first)
//! AC-010.6 Maildir nack returns task to new/
//! AC-010.7 SmartMerger git merge-file fallback (clean + conflict)
//! AC-010.8 WorktreePool allocate/release; non-git fails loudly
//! AC-010.9 MaildirQueue::status counts ready / in_flight / pending
//! AC-010.10 MaildirQueue::reclaim_owner returns cur→new for matching owner

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::json;
use sharecli_fleet::{DeviceRecord, FleetRegistry, DEFAULT_SUBJECT_PREFIX};
use sharecli_mesh::{MaildirQueue, SmartMerger, WorktreePool, WorktreePoolError};
use tempfile::TempDir;

const GIT_LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// FR-010 / AC-010.1 — disconnected registry uses default mesh prefix.
#[test]
fn fr010_default_subject_prefix() {
    let reg = FleetRegistry::disconnected();
    assert_eq!(DEFAULT_SUBJECT_PREFIX, "sharecli.fleet");
    assert_eq!(reg.subject_for("dev-x"), format!("{DEFAULT_SUBJECT_PREFIX}.devices.dev-x"));
}

/// FR-010 / AC-010.2 — custom prefix still yields devices subject.
#[test]
fn fr010_subject_for_with_custom_prefix() {
    let reg = FleetRegistry::disconnected().with_subject_prefix("mesh.lab");
    assert_eq!(reg.subject_for("agent-1"), "mesh.lab.devices.agent-1");
}

/// FR-010 / AC-010.3 — device record JSON round-trips; disconnected register fails.
#[tokio::test]
async fn fr010_device_record_and_register_requires_nats() {
    let rec = DeviceRecord {
        device_id: "dev-mesh-1".into(),
        hostname: "host-a".into(),
        os: "darwin".into(),
        available_slots: 2,
    };
    let json = serde_json::to_string(&rec).expect("serialize");
    let parsed: DeviceRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, rec);
    for key in ["device_id", "hostname", "os", "available_slots"] {
        assert!(json.contains(key), "missing {key}");
    }

    let reg = FleetRegistry::disconnected();
    let err = reg.register(rec).await.expect_err("register without NATS MUST fail loudly");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("not connected") || msg.contains("nats"),
        "expected loud NATS/connect failure, got {msg}"
    );
}

/// FR-010 / AC-010.4 — Maildir enqueue/claim/ack lifecycle (tmp→new→cur).
#[test]
fn fr010_maildir_enqueue_claim_ack() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    let id = q.enqueue(json!({"op": "mesh-task"}), 5).expect("enqueue");
    assert!(dir.path().join("new").join(&id).exists(), "AC-010.4: enqueue MUST land in new/");
    let claimed = q.claim(Some("worker-1")).expect("claim").expect("some");
    assert_eq!(claimed.id, id);
    assert_eq!(claimed.attempts, 1);
    assert!(dir.path().join("cur").join(&id).exists());
    q.ack(&id).expect("ack");
    assert!(q.list_pending().expect("list").is_empty(), "AC-010.4: ack MUST remove from cur/");
}

/// FR-010 / AC-010.5 — lower priority number claimed first.
#[test]
fn fr010_maildir_priority_order() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    q.enqueue(json!("low"), 8).expect("enq low");
    q.enqueue(json!("high"), 1).expect("enq high");
    let first = q.claim(None).expect("claim").expect("some");
    assert_eq!(first.priority, 1, "AC-010.5: priority 1 before 8");
    assert_eq!(first.payload, json!("high"));
}

/// FR-010 / AC-010.6 — nack returns claimed task to new/ for retry.
#[test]
fn fr010_maildir_nack_requeues() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    let id = q.enqueue(json!({}), 4).expect("enq");
    q.claim(None).expect("claim").expect("some");
    q.nack(&id).expect("nack");
    assert!(dir.path().join("new").join(&id).exists(), "AC-010.6: nack MUST restore to new/");
    assert!(!dir.path().join("cur").join(&id).exists());
}

fn write(path: &Path, body: &str) {
    fs::write(path, body).expect("write");
}

/// FR-010 / AC-010.7 — SmartMerger git merge-file fallback (clean merge).
#[test]
fn fr010_smart_merge_git_fallback_clean() {
    let dir = TempDir::new().expect("tempdir");
    let base = dir.path().join("base.txt");
    let ours = dir.path().join("ours.txt");
    let theirs = dir.path().join("theirs.txt");
    let out = dir.path().join("out.txt");
    // Keep a stable middle line so git merge-file treats edits as non-adjacent.
    write(&base, "line1\nshared\nmiddle\nline3\n");
    write(&ours, "line1\nours-edit\nmiddle\nline3\n");
    write(&theirs, "line1\nshared\nmiddle\nline3-theirs\n");

    let merger = SmartMerger::new().with_mergiraf_binary("/nonexistent/mergiraf");
    let result = merger.merge(&base, &ours, &theirs, &out);
    assert!(!result.used_mergiraf, "AC-010.7: must use git fallback");
    assert!(result.success, "AC-010.7: non-overlapping edits MUST succeed: {}", result.output);
    let text = fs::read_to_string(&out).expect("out");
    assert!(text.contains("ours-edit"));
    assert!(text.contains("line3-theirs"));
}

/// FR-010 / AC-010.7 — conflicting edits leave success=false.
#[test]
fn fr010_smart_merge_git_fallback_conflict() {
    let dir = TempDir::new().expect("tempdir");
    let base = dir.path().join("base.txt");
    let ours = dir.path().join("ours.txt");
    let theirs = dir.path().join("theirs.txt");
    let out = dir.path().join("out.txt");
    write(&base, "same\n");
    write(&ours, "ours\n");
    write(&theirs, "theirs\n");

    let merger = SmartMerger::new().with_mergiraf_binary("/nonexistent/mergiraf");
    let result = merger.merge(&base, &ours, &theirs, &out);
    assert!(!result.used_mergiraf);
    assert!(!result.success, "AC-010.7: conflicts MUST set success=false");
    assert!(out.exists());
}

fn git_init_with_commit(dir: &Path) {
    let run = |args: &[&str]| {
        let mut command = Command::new("git");
        command.args(args).current_dir(dir);
        for variable in GIT_LOCAL_ENV_VARS {
            command.env_remove(variable);
        }
        let st = command.output().expect("git");
        assert!(
            st.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&st.stderr)
        );
    };
    run(&["init"]);
    run(&["config", "user.email", "fr010@test"]);
    run(&["config", "user.name", "FR010"]);
    write(&dir.join("README"), "seed\n");
    run(&["add", "README"]);
    run(&["commit", "-m", "seed"]);
}

/// FR-010 / AC-010.8 — WorktreePool allocate/release round-trip.
#[test]
fn fr010_worktree_pool_allocate_release() {
    let repo = TempDir::new().expect("repo");
    git_init_with_commit(repo.path());
    let pool_dir = TempDir::new().expect("pool");
    let pool = WorktreePool::open(repo.path(), pool_dir.path()).expect("open");
    let lease = pool.allocate("slot-1").expect("allocate");
    assert!(
        lease.path.join("README").exists(),
        "AC-010.8: allocated worktree MUST contain repo files"
    );
    pool.release("slot-1").expect("release");
    assert!(!lease.path.exists(), "AC-010.8: release MUST remove worktree path");
}

/// FR-010 / AC-010.8 — non-git pool root fails loudly.
#[test]
fn fr010_worktree_pool_rejects_non_git() {
    let dir = TempDir::new().expect("dir");
    let pool_dir = TempDir::new().expect("pool");
    let err = WorktreePool::open(dir.path(), pool_dir.path()).expect_err("must fail");
    assert!(
        matches!(err, WorktreePoolError::NotGitRepo(_)),
        "AC-010.8: non-git MUST be NotGitRepo, got {err}"
    );
}

/// FR-010 / AC-010.9 — status reports ready / in_flight / pending depths.
#[test]
fn fr010_maildir_status_counts() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    q.enqueue(json!({"a": 1}), 2).expect("enq");
    q.enqueue(json!({"b": 2}), 3).expect("enq");
    q.claim(Some("worker-a")).expect("claim").expect("some");
    let st = q.status().expect("status");
    assert_eq!(st.ready, 1, "AC-010.9: one task remains in new/");
    assert_eq!(st.in_flight, 1, "AC-010.9: one task in cur/");
    assert_eq!(st.pending, 2, "AC-010.9: pending = ready + in_flight");
    assert_eq!(st.path, dir.path());
}

/// FR-010 / AC-010.10 — reclaim_owner moves matching cur/ tasks back to new/.
#[test]
fn fr010_maildir_reclaim_owner() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    let id = q.enqueue(json!({"op": "mesh"}), 1).expect("enq");
    q.claim(Some("dead-agent")).expect("claim").expect("some");
    assert_eq!(
        q.reclaim_owner("other").expect("reclaim other"),
        0,
        "AC-010.10: non-matching owner MUST reclaim 0"
    );
    assert_eq!(
        q.reclaim_owner("dead-agent").expect("reclaim"),
        1,
        "AC-010.10: matching owner MUST reclaim 1"
    );
    assert!(
        dir.path().join("new").join(&id).exists(),
        "AC-010.10: reclaimed task MUST land in new/"
    );
    assert!(!dir.path().join("cur").join(&id).exists());
}
