// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --tree --csv` forest inventory CSV export
//! FR: FR-006
//!
//! AC-006.26 `--tree --csv` emits nested forest rows with root_index and depth

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::render_agent_tree_csv;
use sharecli_fleet::proc_scan::{FakeProcSource, ProcSnapshot};
use sharecli_fleet::AgentTreeNode;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn fixture_forests() -> Vec<AgentTreeNode> {
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
            pid: 51,
            ppid: 50,
            comm: "node".into(),
            cmdline: vec!["node".into()],
            state: 'R',
        },
    ]);
    sharecli_fleet::build_agent_forests(&src)
}

/// FR-006 / AC-006.26 — tree CSV header includes forest nesting columns.
#[test]
fn fr006_proc_tree_csv_header_and_depth() {
    let forests = fixture_forests();
    let mut rss = std::collections::HashMap::new();
    rss.insert(50, 4096_u64);
    let csv = render_agent_tree_csv(&forests, &rss, &HashMap::new(), &HashMap::new());
    let lines: Vec<&str> = csv.trim_end().lines().collect();
    assert_eq!(
        lines[0], "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "header MUST include root_index and depth; got: {}",
        lines[0]
    );
    assert_eq!(lines.len(), 3, "MUST emit root + one child row; got: {csv}");
    assert!(lines[1].starts_with("0,0,50,1,cursor-agent,cursor-agent,,"), "root row: {}", lines[1]);
    assert!(
        lines[2].starts_with("0,1,51,50,,node,,"),
        "child row MUST be depth 1; got: {}",
        lines[2]
    );
}

/// FR-006 / AC-006.26 — multiple forests increment root_index.
#[test]
fn fr006_proc_tree_csv_multiple_roots() {
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
    let forests = sharecli_fleet::build_agent_forests(&src);
    assert_eq!(forests.len(), 2);
    let csv = render_agent_tree_csv(&forests, &HashMap::new(), &HashMap::new(), &HashMap::new());
    let roots: Vec<&str> =
        csv.lines().skip(1).filter(|line| line.split(',').nth(1) == Some("0")).collect();
    assert_eq!(roots.len(), 2);
    assert!(roots[0].starts_with("0,0,10,"));
    assert!(roots[1].starts_with("1,0,20,"));
}

/// FR-006 / AC-006.26 — empty forests emit header only.
#[test]
fn fr006_proc_tree_csv_empty_header_only() {
    let csv = render_agent_tree_csv(&[], &HashMap::new(), &HashMap::new(), &HashMap::new());
    assert_eq!(
        csv.trim_end(),
        "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "empty forests MUST still emit CSV header"
    );
}

/// FR-006 / AC-006.26 — CLI proc --tree --csv succeeds with forest header.
#[test]
fn fr006_proc_tree_csv_cli_exits_zero() {
    let out = bin().args(["proc", "--tree", "--csv"]).output().expect("spawn proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or("");
    assert_eq!(
        first, "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "CLI MUST print forest CSV header; got: {s}"
    );
}

/// FR-006 / AC-006.26 — --tree --csv rejects --json.
#[test]
fn fr006_proc_tree_csv_rejects_json_combo() {
    let out = bin()
        .args(["proc", "--tree", "--csv", "--json"])
        .output()
        .expect("spawn proc --tree --csv --json");
    assert!(!out.status.success());
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("json") || combined.contains("--csv"),
        "MUST reject --tree --csv with --json; got: {combined}"
    );
}
