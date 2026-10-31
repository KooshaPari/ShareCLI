// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --state <letter>` process-state filter
//! FR: FR-006
//!
//! AC-006.31 filter flat inventory and `--tree` by process state (R|S|D|Z|…)

use std::process::Command;

use sharecli::commands::proc::{
    build_agent_state_map, filter_agent_forests, filter_watched_agents, parse_proc_state,
    ProcFilter,
};
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

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch_row(family: &'static str, comm: &str, pid: u32, rss: u64) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: comm.to_string() },
        resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: None },
    }
}

/// FR-006 / AC-006.31 — proc help documents --state.
#[test]
fn fr006_proc_state_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--state"), "proc help MUST document --state; got: {s}");
}

/// FR-006 / AC-006.31 — state filter keeps rows matching the letter (case-insensitive R).
#[test]
fn fr006_proc_state_filter_matches_letter() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot {
            pid: 10,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 11,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
            state: 'S',
        },
    ]);
    let state_by_pid = build_agent_state_map(&src, &[10, 11]);
    let rows = vec![watch_row("claude", "claude", 10, 100), watch_row("codex", "codex", 11, 200)];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter { state: Some('R'), ..Default::default() },
        &empty_ppid_map(),
        &empty_cmdline_map(),
        &state_by_pid,
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.31 — state filter composes with --cmdline and --comm.
#[test]
fn fr006_proc_state_composes_with_cmdline_and_comm() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot {
            pid: 10,
            ppid: 1,
            comm: "node".into(),
            cmdline: vec!["node".into(), "cursor-agent".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 11,
            ppid: 1,
            comm: "node".into(),
            cmdline: vec!["node".into(), "webpack".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 12,
            ppid: 1,
            comm: "node".into(),
            cmdline: vec!["node".into(), "cursor-agent".into()],
            state: 'S',
        },
    ]);
    let cmdline_by_pid = sharecli::commands::proc::build_agent_cmdline_map(&src, &[10, 11, 12]);
    let state_by_pid = build_agent_state_map(&src, &[10, 11, 12]);
    let rows = vec![
        watch_row("cursor", "node", 10, 100),
        watch_row("cursor", "node", 11, 100),
        watch_row("cursor", "node", 12, 100),
    ];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter {
            comm: Some("node".into()),
            cmdline: Some("cursor".into()),
            state: Some('R'),
            ..Default::default()
        },
        &empty_ppid_map(),
        &cmdline_by_pid,
        &state_by_pid,
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.31 — tree root forests honor state filter on roots.
#[test]
fn fr006_proc_tree_state_filter() {
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
            state: 'S',
        },
    ]);
    let state_by_pid = build_agent_state_map(&src, &[50, 60]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    assert_eq!(forests.len(), 2);
    let filtered = filter_agent_forests(
        &forests,
        &ProcFilter { state: Some('S'), ..Default::default() },
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &empty_cmdline_map(),
        &state_by_pid,
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].comm, "codex");
}

/// FR-006 / AC-006.31 — parse_proc_state accepts lowercase sleeping.
#[test]
fn fr006_proc_state_parse_case_insensitive_common() {
    assert_eq!(parse_proc_state("s").unwrap(), 'S');
    assert_eq!(parse_proc_state("R").unwrap(), 'R');
}

/// FR-006 / AC-006.31 — empty state fails loudly.
#[test]
fn fr006_proc_empty_state_rejected() {
    let out = bin().args(["proc", "--state", ""]).output().expect("spawn sharecli proc --state");
    assert!(!out.status.success(), "empty --state MUST fail");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("state"),
        "error MUST mention --state; got: {combined}"
    );
}

/// FR-006 / AC-006.31 — invalid state token fails loudly.
#[test]
fn fr006_proc_invalid_state_rejected() {
    assert!(parse_proc_state("").is_err());
    assert!(parse_proc_state("RS").is_err());
    assert!(parse_proc_state("9").is_err());
    let out = bin().args(["proc", "--state", "9"]).output().expect("spawn sharecli proc --state 9");
    assert!(!out.status.success(), "invalid --state MUST fail");
}
