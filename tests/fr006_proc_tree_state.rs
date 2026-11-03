// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --tree` process-state on text nodes and tree JSON
//! FR: FR-006
//!
//! AC-006.34 expose `state` on `--tree` text nodes and `--tree --json`
//! `AgentTreeNodeJson` rows (tree CSV already has state from AC-006.32)
//! AC-006.35 live tree state lookup covers all forest PIDs (roots + children)

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::{
    agent_tree_node_to_json, build_agent_state_map, build_forest_state_map, render_agent_tree_csv,
    state_text_for_pid,
};
use sharecli_fleet::collect_forest_pids;
use sharecli_fleet::proc_scan::{FakeProcSource, ProcSnapshot};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-006 / AC-006.34 — tree JSON nodes include state letter.
#[test]
fn fr006_proc_tree_state_json_node_includes_state() {
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
    let root = agent_tree_node_to_json(&forests[0], &state_by_pid);
    assert_eq!(root.state, "S");
    assert_eq!(root.children[0].state, "R");
    let json = serde_json::to_string(&root).expect("serialize tree node");
    assert!(json.contains("\"state\":\"S\""), "tree JSON MUST include state on root; got: {json}");
}

/// FR-006 / AC-006.34 — missing tree JSON state serializes as empty string.
#[test]
fn fr006_proc_tree_state_json_missing_state_empty_string() {
    let src = FakeProcSource::new(vec![ProcSnapshot {
        pid: 99,
        ppid: 1,
        comm: "codex".into(),
        cmdline: vec!["codex".into()],
        state: 'R',
    }]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    let node = agent_tree_node_to_json(&forests[0], &HashMap::new());
    assert_eq!(node.state, "");
    let json = serde_json::to_string(&node).expect("serialize tree node");
    assert!(
        json.contains("\"state\":\"\""),
        "missing state MUST be empty JSON string; got: {json}"
    );
}

/// FR-006 / AC-006.34 — CLI proc --tree --json forests nodes expose state key.
#[test]
fn fr006_proc_tree_state_cli_json_includes_state_key() {
    let out = bin().args(["proc", "--tree", "--json"]).output().expect("spawn proc --tree --json");
    assert!(out.status.success(), "proc --tree --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --tree --json MUST emit valid JSON");
    let forests = v.get("forests").and_then(|f| f.as_array()).expect("forests array");
    if let Some(root) = forests.first() {
        assert!(
            root.get("state").is_some(),
            "tree JSON root MUST include state when forests present; got: {root}"
        );
    }
}

/// FR-006 / AC-006.35 — collect_forest_pids walks roots and nested children.
#[test]
fn fr006_proc_tree_forest_state_collects_all_pids() {
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
        ProcSnapshot {
            pid: 52,
            ppid: 51,
            comm: "bash".into(),
            cmdline: vec!["bash".into()],
            state: 'D',
        },
    ]);
    let forests = sharecli_fleet::build_agent_forests(&src);
    let mut pids = collect_forest_pids(&forests);
    pids.sort_unstable();
    assert_eq!(pids, vec![50, 51, 52]);
}

/// FR-006 / AC-006.35 — build_forest_state_map resolves child states not in agent scan list.
#[test]
fn fr006_proc_tree_forest_state_map_includes_child_states() {
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
    let agent_only = build_agent_state_map(&src, &[50]);
    assert_eq!(agent_only.get(&51), None, "agent-only map MUST omit child PID");
    let forest_state = build_forest_state_map(&src, &forests);
    assert_eq!(forest_state.get(&50), Some(&'S'));
    assert_eq!(forest_state.get(&51), Some(&'R'));
    let root = agent_tree_node_to_json(&forests[0], &forest_state);
    assert_eq!(root.children[0].state, "R");
}

/// FR-006 / AC-006.35 — tree CSV child rows get state from forest lookup.
#[test]
fn fr006_proc_tree_forest_state_csv_child_row_includes_state() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
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
    let state_by_pid = build_forest_state_map(&src, &forests);
    let csv = render_agent_tree_csv(&forests, &HashMap::new(), &HashMap::new(), &state_by_pid);
    assert!(
        csv.contains(",51,50,") && csv.contains(",R,"),
        "tree CSV child row MUST include state letter; got: {csv}"
    );
}

/// FR-006 / AC-006.35 — tree text child nodes show state from forest lookup.
#[test]
fn fr006_proc_tree_forest_state_text_child_node_shows_state() {
    let src = FakeProcSource::new(vec![
        ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
        ProcSnapshot {
            pid: 50,
            ppid: 1,
            comm: "codex".into(),
            cmdline: vec!["codex".into()],
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
    let agent_only = build_agent_state_map(&src, &[50]);
    assert_eq!(state_text_for_pid(&agent_only, 51), "-", "agent-only map shows `-` for child");
    let forest_state = build_forest_state_map(&src, &forests);
    assert_eq!(state_text_for_pid(&forest_state, 51), "R");
}

/// FR-006 / AC-006.34 — CLI proc --tree text nodes show state letter after PID.
#[test]
fn fr006_proc_tree_state_cli_text_shows_state_on_nodes() {
    let out = bin().args(["proc", "--tree"]).output().expect("spawn proc --tree");
    assert!(out.status.success(), "proc --tree MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    // Only run the state-letter check when the output actually renders PID
    // tree nodes (e.g. "[100] S claude"). The output always contains a
    // bracketed gate decision like "[ADMIT]" plus numeric meter values, so
    // the guard must require a digit immediately after a '['.
    let has_pid_nodes =
        s.split('[').skip(1).any(|seg| seg.starts_with(|c: char| c.is_ascii_digit()));
    if has_pid_nodes {
        assert!(
            s.contains("] ")
                && (s.contains("] R ")
                    || s.contains("] S ")
                    || s.contains("] D ")
                    || s.contains("] Z ")
                    || s.contains("] T ")
                    || s.contains("] - ")),
            "tree text nodes MUST show state after PID bracket; got: {s}"
        );
    }
}
