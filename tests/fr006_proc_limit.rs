// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --limit`
//! FR: FR-006
//!
//! AC-006.21 `--limit N` caps flat inventory rows and tree root forests after
//! filter/sort; invalid limits fail loudly.

use std::process::Command;

use sharecli::commands::proc::{limit_agent_forests, limit_watched_agents, parse_proc_limit};
use sharecli_fleet::{
    proc_scan::{AgentTreeNode, DetectedAgent},
    AgentResourceSample, DetectedAgentWatch,
};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch(pid: u32, family: &'static str) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: family.into() },
        resource: AgentResourceSample { mem_rss_bytes: pid as u64 * 100, fd_count: None },
    }
}

fn tree_root(pid: u32) -> AgentTreeNode {
    AgentTreeNode { pid, ppid: 1, comm: "agent".into(), family: Some("claude"), children: vec![] }
}

/// FR-006 / AC-006.21 — proc help documents --limit.
#[test]
fn fr006_proc_help_documents_limit() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success(), "proc --help should exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--limit"), "proc --help MUST document --limit; got: {s}");
}

/// FR-006 / AC-006.21 — limit keeps first N rows after sort order is applied.
#[test]
fn fr006_proc_limit_caps_watched_rows() {
    let rows = vec![watch(10, "claude"), watch(20, "codex"), watch(30, "aider")];
    let limited = limit_watched_agents(rows, Some(2));
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].agent.pid, 10);
    assert_eq!(limited[1].agent.pid, 20);
}

/// FR-006 / AC-006.21 — limit larger than inventory is a no-op.
#[test]
fn fr006_proc_limit_noop_when_exceeds_inventory() {
    let rows = vec![watch(1, "claude")];
    let limited = limit_watched_agents(rows, Some(99));
    assert_eq!(limited.len(), 1);
}

/// FR-006 / AC-006.21 — tree root forests honor the same cap.
#[test]
fn fr006_proc_limit_caps_tree_roots() {
    let forests = vec![tree_root(1), tree_root(2), tree_root(3)];
    let limited = limit_agent_forests(forests, Some(1));
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].pid, 1);
}

/// FR-006 / AC-006.21 — zero limit is rejected.
#[test]
fn fr006_proc_limit_zero_rejected() {
    let err = parse_proc_limit(Some(0)).expect_err("limit 0 MUST fail");
    assert!(
        err.to_string().contains(">= 1") || err.to_string().contains("limit"),
        "error MUST mention minimum limit; got: {err}"
    );
}

/// FR-006 / AC-006.21 — CLI rejects --limit 0.
#[test]
fn fr006_proc_limit_zero_cli_exits_nonzero() {
    let out = bin().args(["proc", "--limit", "0"]).output().expect("spawn sharecli proc --limit 0");
    assert!(!out.status.success(), "proc --limit 0 MUST fail");
}
