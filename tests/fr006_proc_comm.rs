// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --comm <pattern>` COMM substring filter
//! FR: FR-006
//!
//! AC-006.29 case-insensitive substring match on process comm

use std::process::Command;

use sharecli::commands::proc::{filter_agent_forests, filter_watched_agents, ProcFilter};
use sharecli_fleet::{
    proc_scan::{DetectedAgent, FakeProcSource, ProcSnapshot},
    AgentResourceSample, DetectedAgentWatch,
};

fn empty_ppid_map() -> std::collections::HashMap<u32, u32> {
    std::collections::HashMap::new()
}

fn empty_cmdline_map() -> std::collections::HashMap<u32, String> {
    std::collections::HashMap::new()
}

fn empty_state_map() -> std::collections::HashMap<u32, char> {
    std::collections::HashMap::new()
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch_row_comm(family: &'static str, comm: &str, pid: u32, rss: u64) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: comm.to_string() },
        resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: None },
    }
}

/// FR-006 / AC-006.29 — proc help documents --comm.
#[test]
fn fr006_proc_comm_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--comm"), "proc help MUST document --comm; got: {s}");
}

/// FR-006 / AC-006.29 — comm filter is case-insensitive substring on flat inventory.
#[test]
fn fr006_proc_comm_filter_substring_case_insensitive() {
    let rows = vec![
        watch_row_comm("claude", "Claude-Code", 10, 100),
        watch_row_comm("codex", "codex", 11, 200),
        watch_row_comm("amp", "node", 12, 300),
    ];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter { comm: Some("-Code".into()), ..Default::default() },
        &empty_ppid_map(),
        &empty_cmdline_map(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.29 — comm filter composes with family filter.
#[test]
fn fr006_proc_comm_composes_with_family() {
    let rows = vec![
        watch_row_comm("claude", "Claude-Code", 10, 100),
        watch_row_comm("claude", "claude", 11, 100),
        watch_row_comm("codex", "codex-cli", 12, 100),
    ];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter {
            family: Some("claude".into()),
            comm: Some("code".into()),
            ..Default::default()
        },
        &empty_ppid_map(),
        &empty_cmdline_map(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.29 — tree root forests honor comm filter on roots.
#[test]
fn fr006_proc_tree_comm_filter() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "cursor-agent".into(),
            cmdline: vec!["cursor-agent".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 60,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
            state: 'R',
        },
    ]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    assert_eq!(forests.len(), 2);
    let filtered = filter_agent_forests(
        &forests,
        &ProcFilter { comm: Some("CURSOR".into()), ..Default::default() },
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].comm, "cursor-agent");
}

/// FR-006 / AC-006.29 — empty comm pattern fails loudly.
#[test]
fn fr006_proc_empty_comm_rejected() {
    let out = bin().args(["proc", "--comm", ""]).output().expect("spawn sharecli proc --comm");
    assert!(!out.status.success(), "empty --comm MUST fail");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("comm"),
        "error MUST mention --comm; got: {combined}"
    );
}
