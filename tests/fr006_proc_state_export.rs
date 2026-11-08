// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc` process-state export on JSON/CSV surfaces
//! FR: FR-006
//!
//! AC-006.32 expose `state` on flat `--json` agent rows and `--csv` columns
//! (and `--tree --csv`) so operators see the state letter without re-scanning

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::{
    agent_row_from_watch, render_agent_inventory_csv, render_agent_tree_csv, AgentProcRow,
};
use sharecli_fleet::proc_scan::{FakeProcSource, ProcSnapshot};
use sharecli_fleet::{AgentResourceSample, DetectedAgent, DetectedAgentWatch};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn watch(
    pid: u32,
    family: &'static str,
    comm: &'static str,
    rss: u64,
    fds: Option<u64>,
) -> DetectedAgentWatch {
    DetectedAgentWatch {
        agent: DetectedAgent { pid, family, comm: comm.into() },
        resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: fds },
    }
}

/// FR-006 / AC-006.32 — flat JSON agent rows include state letter.
#[test]
fn fr006_proc_state_export_json_agent_row_includes_state() {
    let row = watch(42, "claude", "claude", 4096, Some(7));
    let mut state_by_pid = HashMap::new();
    state_by_pid.insert(42, 'S');
    let agent = agent_row_from_watch(&row, &state_by_pid);
    assert_eq!(
        agent,
        AgentProcRow {
            pid: 42,
            family: "claude".into(),
            comm: "claude".into(),
            state: "S".into(),
            mem_rss_bytes: 4096,
            mem_rss: agent.mem_rss.clone(),
            fd_count: Some(7),
        }
    );
    let json = serde_json::to_string(&agent).expect("serialize AgentProcRow");
    assert!(json.contains("\"state\":\"S\""), "JSON MUST include state field; got: {json}");
}

/// FR-006 / AC-006.32 — flat CSV header and rows include state column.
#[test]
fn fr006_proc_state_export_flat_csv_includes_state_column() {
    let rows = vec![watch(42, "claude", "claude", 4096, Some(7))];
    let mut state_by_pid = HashMap::new();
    state_by_pid.insert(42, 'R');
    let csv = render_agent_inventory_csv(&rows, &state_by_pid);
    let lines: Vec<&str> = csv.trim_end().lines().collect();
    assert_eq!(
        lines[0], "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "flat CSV header MUST include state column; got: {}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("42,claude,claude,R,4096,"),
        "flat CSV row MUST include state letter; got: {}",
        lines[1]
    );
}

/// FR-006 / AC-006.32 — tree CSV header and rows include state column.
#[test]
fn fr006_proc_state_export_tree_csv_includes_state_column() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "cursor-agent".into(),
            cmdline: vec!["cursor-agent".into()],
            state: 'S',
        },
        ProcSnapshot {
            pid: 51,
            ppid: 50,
            comm: "node".into(),
            cmdline: vec!["node".into()],
            state: 'R',
        },
    ]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    let mut state_by_pid = HashMap::new();
    state_by_pid.insert(50, 'S');
    state_by_pid.insert(51, 'R');
    let csv = render_agent_tree_csv(&forests, &HashMap::new(), &HashMap::new(), &state_by_pid);
    let lines: Vec<&str> = csv.trim_end().lines().collect();
    assert_eq!(
        lines[0], "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "tree CSV header MUST include state column; got: {}",
        lines[0]
    );
    assert!(
        lines[1].contains(",cursor-agent,S,"),
        "root tree row MUST include state S; got: {}",
        lines[1]
    );
    assert!(
        lines[2].contains(",node,R,"),
        "child tree row MUST include state R; got: {}",
        lines[2]
    );
}

/// FR-006 / AC-006.32 — missing state leaves empty CSV field.
#[test]
fn fr006_proc_state_export_csv_missing_state_empty_field() {
    let rows = vec![watch(99, "codex", "codex", 100, None)];
    let csv = render_agent_inventory_csv(&rows, &HashMap::new());
    assert!(
        csv.contains("99,codex,codex,,100,"),
        "missing state MUST leave empty CSV field; got: {csv}"
    );
}

/// FR-006 / AC-006.32 — CLI proc --json agent objects expose state key.
#[test]
fn fr006_proc_state_export_cli_json_includes_state_key() {
    let out = bin().args(["proc", "--json"]).output().expect("spawn proc --json");
    assert!(out.status.success(), "proc --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --json MUST emit valid JSON");
    let agents = v.get("agents").and_then(|a| a.as_array()).expect("agents array");
    if let Some(first) = agents.first() {
        assert!(
            first.get("state").is_some(),
            "agent JSON objects MUST include state when agents present; got: {first}"
        );
    }
}

/// FR-006 / AC-006.32 — CLI flat --csv header includes state column.
#[test]
fn fr006_proc_state_export_cli_flat_csv_header() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    let first = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").to_string();
    assert_eq!(
        first, "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "CLI flat CSV MUST include state column; got: {first}"
    );
}

/// FR-006 / AC-006.32 — CLI tree --csv header includes state column.
#[test]
fn fr006_proc_state_export_cli_tree_csv_header() {
    let out = bin().args(["proc", "--tree", "--csv"]).output().expect("spawn proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    let first = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").to_string();
    assert_eq!(
        first, "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "CLI tree CSV MUST include state column; got: {first}"
    );
}

/// FR-006 / AC-006.32 — --state filter composes with JSON/CSV export surfaces.
#[test]
fn fr006_proc_state_export_composes_with_state_filter() {
    let out =
        bin().args(["proc", "--state", "S", "--csv"]).output().expect("spawn proc --state S --csv");
    assert!(out.status.success(), "proc --state S --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    let header = s.lines().next().unwrap_or("");
    assert!(header.contains("state"), "filtered CSV MUST still expose state column; got: {s}");
}
