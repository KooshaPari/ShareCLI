// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --pid N` process detail view
//! FR: FR-006
//!
//! AC-006.23 `--pid` shows RSS/FD/cmdline/parent for a live host process

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-006 / AC-006.23 — proc help documents --pid.
#[test]
fn fr006_proc_pid_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--pid"), "proc help MUST document --pid; got: {s}");
}

/// FR-006 / AC-006.23 — unknown PID exits non-zero.
#[test]
fn fr006_proc_pid_missing_process_fails() {
    let out = bin().args(["proc", "--pid", "999999999"]).output().expect("spawn proc --pid");
    assert!(!out.status.success(), "missing PID MUST exit non-zero");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("999999999") || err.contains("not found"),
        "error MUST mention missing PID; got: {err}"
    );
}

/// FR-006 / AC-006.23 — self PID detail includes RSS, cmdline, parent fields.
#[test]
fn fr006_proc_pid_detail_self_json_shape() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json"])
        .output()
        .expect("spawn proc --pid --json");
    assert!(out.status.success(), "proc --pid self MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --pid --json MUST emit valid JSON");
    assert_eq!(v.get("pid").and_then(|p| p.as_u64()), Some(pid as u64));
    assert!(v.get("ppid").and_then(|p| p.as_u64()).is_some());
    assert!(v.get("comm").and_then(|c| c.as_str()).is_some());
    assert!(v.get("cmdline").and_then(|c| c.as_array()).is_some());
    assert!(v.get("mem_rss_bytes").and_then(|r| r.as_u64()).is_some());
    assert!(v.get("mem_rss").and_then(|r| r.as_str()).is_some());
    assert!(v.get("state").is_some(), "proc detail JSON MUST include state key; got: {v}");
}

/// FR-006 / AC-006.23 — text detail prints parent and cmdline sections.
#[test]
fn fr006_proc_pid_detail_self_text_sections() {
    let pid = std::process::id();
    let out = bin().args(["proc", "--pid", &pid.to_string()]).output().expect("spawn proc --pid");
    assert!(out.status.success(), "proc --pid self MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("Process detail"), "MUST print detail header; got: {s}");
    assert!(s.contains("Parent:"), "MUST print parent line; got: {s}");
    assert!(s.contains("CMDLINE:"), "MUST print cmdline line; got: {s}");
    assert!(s.contains("RSS:"), "MUST print RSS line; got: {s}");
    assert!(s.contains("State:"), "MUST print State line; got: {s}");
}
