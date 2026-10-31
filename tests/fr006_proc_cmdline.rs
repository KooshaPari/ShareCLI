// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --cmdline <pattern>` cmdline substring filter
//! FR: FR-006
//!
//! AC-006.30 case-insensitive substring match on joined argv/cmdline

use std::process::Command;

use sharecli::commands::proc::{
    build_agent_cmdline_map, filter_agent_forests, filter_watched_agents, ProcFilter,
};
use sharecli_fleet::{
    proc_scan::{DetectedAgent, FakeProcSource, ProcSnapshot},
    AgentResourceSample, DetectedAgentWatch,
};

fn empty_ppid_map() -> std::collections::HashMap<u32, u32> {
    std::collections::HashMap::new()
}

fn empty_state_map() -> std::collections::HashMap<u32, char> {
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

/// FR-006 / AC-006.30 — proc help documents --cmdline.
#[test]
fn fr006_proc_cmdline_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--cmdline"), "proc help MUST document --cmdline; got: {s}");
}

/// FR-006 / AC-006.30 — cmdline filter is case-insensitive substring on joined argv.
#[test]
fn fr006_proc_cmdline_filter_substring_case_insensitive() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot {
            pid: 10,
            ppid: 1,
            comm: "node".into(),
            cmdline: vec!["node".into(), "conversation".into(), "list".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 11,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into(), "exec".into()],
            state: 'R',
        },
    ]);
    let cmdline_by_pid = build_agent_cmdline_map(&src, &[10, 11]);
    let rows = vec![watch_row("forge", "node", 10, 100), watch_row("codex", "codex", 11, 200)];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter { cmdline: Some("CONVERSATION".into()), ..Default::default() },
        &empty_ppid_map(),
        &cmdline_by_pid,
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.30 — cmdline filter composes with --comm.
#[test]
fn fr006_proc_cmdline_composes_with_comm() {
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
    ]);
    let cmdline_by_pid = build_agent_cmdline_map(&src, &[10, 11]);
    let rows = vec![watch_row("cursor", "node", 10, 100), watch_row("cursor", "node", 11, 100)];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter {
            comm: Some("node".into()),
            cmdline: Some("cursor".into()),
            ..Default::default()
        },
        &empty_ppid_map(),
        &cmdline_by_pid,
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.pid, 10);
}

/// FR-006 / AC-006.30 — tree root forests honor cmdline filter on roots.
#[test]
fn fr006_proc_tree_cmdline_filter() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "cursor-agent".into(),
            cmdline: vec!["cursor-agent".into(), "--headless".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 60,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into(), "exec".into()],
            state: 'R',
        },
    ]);
    let cmdline_by_pid = build_agent_cmdline_map(&src, &[50, 60]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    assert_eq!(forests.len(), 2);
    let filtered = filter_agent_forests(
        &forests,
        &ProcFilter { cmdline: Some("headless".into()), ..Default::default() },
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &cmdline_by_pid,
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].comm, "cursor-agent");
}

/// FR-006 / AC-006.30 — empty cmdline pattern fails loudly.
#[test]
fn fr006_proc_empty_cmdline_rejected() {
    let out =
        bin().args(["proc", "--cmdline", ""]).output().expect("spawn sharecli proc --cmdline");
    assert!(!out.status.success(), "empty --cmdline MUST fail");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("cmdline"),
        "error MUST mention --cmdline; got: {combined}"
    );
}
