// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-010 — mesh CLI status / reclaim (operator surface)
//! FR: FR-010
//!
//! AC-010.9 CLI `mesh status --queue` reports depths
//! AC-010.10 CLI `mesh reclaim --queue --owner` returns cur→new

use std::process::Command;

use serde_json::json;
use sharecli_mesh::MaildirQueue;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// FR-010 / AC-010.9 — `sharecli mesh status --json` mirrors MaildirStatus.
#[test]
fn fr010_cli_mesh_status_json() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    q.enqueue(json!({"cli": true}), 1).expect("enq");
    q.claim(Some("cli-worker")).expect("claim").expect("some");

    let out = bin()
        .args(["mesh", "status", "--queue"])
        .arg(dir.path())
        .arg("--json")
        .output()
        .expect("spawn mesh status");
    assert!(out.status.success(), "mesh status MUST exit 0; stderr={}", stderr(&out));
    let body = stdout(&out);
    let v: serde_json::Value = serde_json::from_str(&body).expect("json status");
    assert_eq!(v["ready"], 0);
    assert_eq!(v["in_flight"], 1);
    assert_eq!(v["pending"], 1);
}

/// FR-010 / AC-010.10 — `sharecli mesh reclaim` returns owned cur/ tasks.
#[test]
fn fr010_cli_mesh_reclaim() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    let id = q.enqueue(json!({"cli": "reclaim"}), 2).expect("enq");
    q.claim(Some("stranded")).expect("claim").expect("some");

    let out = bin()
        .args(["mesh", "reclaim", "--queue"])
        .arg(dir.path())
        .args(["--owner", "stranded"])
        .output()
        .expect("spawn mesh reclaim");
    assert!(out.status.success(), "mesh reclaim MUST exit 0; stderr={}", stderr(&out));
    let body = stdout(&out);
    assert!(body.contains("reclaimed 1"), "expected reclaim count in stdout, got {body}");
    assert!(dir.path().join("new").join(&id).exists(), "AC-010.10 CLI: task MUST be back in new/");
}

/// FR-010 — empty --owner fails loudly (no silent reclaim of everything).
#[test]
fn fr010_cli_mesh_reclaim_empty_owner_fails() {
    let dir = TempDir::new().expect("tempdir");
    let _q = MaildirQueue::open(dir.path()).expect("open");
    let out = bin()
        .args(["mesh", "reclaim", "--queue"])
        .arg(dir.path())
        .args(["--owner", "   "])
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "empty owner MUST fail; stdout={}", stdout(&out));
}
