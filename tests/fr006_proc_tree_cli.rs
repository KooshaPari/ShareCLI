// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --tree` parent-child agent forests
//! FR: FR-006
//!
//! AC-006.16 `sharecli proc --tree` renders agent-rooted process subtrees

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-006 / AC-006.16 — proc help documents --tree.
#[test]
fn fr006_proc_help_documents_tree() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success(), "proc --help should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--tree"), "proc --help MUST document --tree; got: {s}");
}

/// FR-006 / AC-006.16 — proc --tree prints forest header.
#[test]
fn fr006_proc_tree_prints_forest_header() {
    let out = bin().args(["proc", "--tree"]).output().expect("spawn sharecli proc --tree");
    assert!(out.status.success(), "proc --tree should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("Agent process tree (proc scan)"),
        "proc --tree MUST print forest header; got: {s}"
    );
}

/// FR-006 / AC-006.16 — proc --tree --json emits nested forests array.
#[test]
fn fr006_proc_tree_json_shape() {
    let out = bin()
        .args(["proc", "--tree", "--json"])
        .output()
        .expect("spawn sharecli proc --tree --json");
    assert!(out.status.success(), "proc --tree --json should exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --tree --json MUST emit valid JSON");
    assert!(v.get("forests").and_then(|f| f.as_array()).is_some());
    assert!(v.get("roots").and_then(|r| r.as_u64()).is_some());
}
