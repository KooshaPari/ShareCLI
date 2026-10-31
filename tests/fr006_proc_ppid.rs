// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --ppid N` parent-PID inventory filter
//! FR: FR-006
//!
//! AC-006.25 `--ppid` keeps agents whose parent PID matches N

use std::process::Command;

use sharecli::commands::proc::{
    build_agent_ppid_map, filter_agent_forests, filter_watched_agents, ProcFilter,
};
use sharecli_fleet::{
    proc_scan::{DetectedAgent, FakeProcSource, ProcSnapshot},
    AgentResourceSample, DetectedAgentWatch,
};

fn empty_cmdline_map() -> std::collections::HashMap<u32, String> {
    std::collections::HashMap::new()
}

fn empty_state_map() -> std::collections::HashMap<u32, char> {
    std::collections::HashMap::new()
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch_row(family: &'static str, pid: u32, rss: u64) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: family.into() },
        resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: None },
    }
}

/// FR-006 / AC-006.25 — proc help documents --ppid.
#[test]
fn fr006_proc_ppid_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--ppid"), "proc help MUST document --ppid; got: {s}");
}

/// FR-006 / AC-006.25 — ppid filter keeps agents with matching parent PID.
#[test]
fn fr006_proc_ppid_filter_keeps_matching_parent() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 10,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 20,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 30,
            ppid: 10,
            comm: "forge".into(),
            cmdline: vec!["forge".into(), "conversation".into(), "list".into()],
            state: 'R',
        },
    ]);
    let ppid_map = build_agent_ppid_map(&src, &[10, 20, 30]);
    let rows = vec![
        watch_row("claude", 10, 100),
        watch_row("codex", 20, 100),
        watch_row("forge", 30, 100),
    ];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter {
            ppid: Some(1),
            family: None,
            exclude_family: None,
            min_rss_bytes: None,
            max_rss_bytes: None,
            min_fd_count: None,
            max_fd_count: None,
            comm: None,
            cmdline: None,
            state: None,
        },
        &ppid_map,
        &empty_cmdline_map(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|r| r.agent.pid == 10));
    assert!(filtered.iter().any(|r| r.agent.pid == 20));
    assert!(!filtered.iter().any(|r| r.agent.pid == 30));
}

/// FR-006 / AC-006.25 — ppid filter composes with family filter.
#[test]
fn fr006_proc_ppid_composes_with_family() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 10,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        },
        ProcSnapshot {
            pid: 20,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
            state: 'R',
        },
    ]);
    let ppid_map = build_agent_ppid_map(&src, &[10, 20]);
    let rows = vec![watch_row("claude", 10, 100), watch_row("codex", 20, 100)];
    let filtered = filter_watched_agents(
        &rows,
        &ProcFilter {
            ppid: Some(1),
            family: Some("claude".into()),
            exclude_family: None,
            min_rss_bytes: None,
            max_rss_bytes: None,
            min_fd_count: None,
            max_fd_count: None,
            comm: None,
            cmdline: None,
            state: None,
        },
        &ppid_map,
        &empty_cmdline_map(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].agent.family, "claude");
}

/// FR-006 / AC-006.25 — tree root forests honor ppid filter on roots.
#[test]
fn fr006_proc_tree_ppid_filter() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
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
        &ProcFilter {
            ppid: Some(1),
            family: None,
            exclude_family: None,
            min_rss_bytes: None,
            max_rss_bytes: None,
            min_fd_count: None,
            max_fd_count: None,
            comm: None,
            cmdline: None,
            state: None,
        },
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &empty_state_map(),
    );
    assert_eq!(filtered.len(), 2);
    let filtered_launchd = filter_agent_forests(
        &forests,
        &ProcFilter {
            ppid: Some(999),
            family: None,
            exclude_family: None,
            min_rss_bytes: None,
            max_rss_bytes: None,
            min_fd_count: None,
            max_fd_count: None,
            comm: None,
            cmdline: None,
            state: None,
        },
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &empty_state_map(),
    );
    assert!(filtered_launchd.is_empty());
}

/// FR-006 / AC-006.25 — --ppid cannot combine with --pid.
#[test]
fn fr006_proc_ppid_rejects_pid_combo() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--ppid", "1", "--pid", &pid.to_string()])
        .output()
        .expect("spawn proc --ppid --pid");
    assert!(!out.status.success());
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("ppid") || combined.contains("--pid"),
        "MUST reject --ppid with --pid; got: {combined}"
    );
}

/// FR-006 / AC-006.25 — CLI proc --ppid succeeds (may yield empty inventory).
#[test]
fn fr006_proc_ppid_cli_exits_zero() {
    let out = bin().args(["proc", "--ppid", "1"]).output().expect("spawn proc --ppid");
    assert!(out.status.success(), "proc --ppid MUST exit 0; stderr: {:?}", out.stderr);
}

/// FR-006 / AC-006.25 — --ppid with --json emits structured inventory.
#[test]
fn fr006_proc_ppid_json_shape() {
    let out =
        bin().args(["proc", "--ppid", "1", "--json"]).output().expect("spawn proc --ppid --json");
    assert!(out.status.success(), "proc --ppid --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --ppid --json MUST emit valid JSON");
    assert!(v.get("agents").and_then(|a| a.as_array()).is_some());
    assert!(v.get("gate").is_some());
}
