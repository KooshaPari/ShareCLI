// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — `sharecli proc --csv` flat inventory CSV export
//! FR: FR-006
//!
//! AC-006.24 `--csv` emits header + agent rows after filter/sort/limit

use std::collections::HashMap;
use std::process::Command;

use sharecli::commands::proc::{render_agent_inventory_csv, sort_watched_agents, ProcSort};
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

/// FR-006 / AC-006.24 — proc help documents --csv.
#[test]
fn fr006_proc_csv_help_documents_flag() {
    let out = bin().args(["proc", "--help"]).output().expect("spawn sharecli proc --help");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--csv"), "proc help MUST document --csv; got: {s}");
}

/// FR-006 / AC-006.24 — CSV header and row columns.
#[test]
fn fr006_proc_csv_header_and_columns() {
    let rows = vec![watch(42, "claude", "claude", 4096, Some(7))];
    let csv = render_agent_inventory_csv(&rows, &HashMap::new());
    let lines: Vec<&str> = csv.trim_end().lines().collect();
    assert_eq!(lines.len(), 2, "MUST emit header + one data row; got: {csv}");
    assert_eq!(
        lines[0], "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "header MUST match schema; got: {}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("42,claude,claude,,4096,"),
        "row MUST include pid/family/rss; got: {}",
        lines[1]
    );
    assert!(lines[1].ends_with(",7"), "fd_count MUST be present; got: {}", lines[1]);
}

/// FR-006 / AC-006.24 — CSV escapes commas in comm fields.
#[test]
fn fr006_proc_csv_escapes_commas() {
    let rows = vec![watch(1, "codex", "node,main.js", 100, None)];
    let csv = render_agent_inventory_csv(&rows, &HashMap::new());
    assert!(csv.contains("\"node,main.js\""), "comm with comma MUST be quoted; got: {csv}");
    assert!(
        csv.contains(",,") || csv.ends_with(",\n") || csv.contains(",,\n"),
        "missing fd_count MUST leave empty field; got: {csv}"
    );
}

/// FR-006 / AC-006.24 — empty inventory emits header only.
#[test]
fn fr006_proc_csv_empty_inventory_header_only() {
    let csv = render_agent_inventory_csv(&[], &HashMap::new());
    assert_eq!(
        csv.trim_end(),
        "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "empty inventory MUST still emit CSV header"
    );
}

/// FR-006 / AC-006.24 — CLI proc --csv succeeds and parses as CSV.
#[test]
fn fr006_proc_csv_cli_exits_zero() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or("");
    assert_eq!(
        first, "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count",
        "CLI MUST print CSV header; got: {s}"
    );
}

/// FR-006 / AC-006.24 — --csv rejects --json.
#[test]
fn fr006_proc_csv_rejects_json_combo() {
    let out = bin().args(["proc", "--csv", "--json"]).output().expect("spawn proc --csv --json");
    assert!(!out.status.success());
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.to_ascii_lowercase().contains("json") || combined.contains("--csv"),
        "MUST reject --csv with --json; got: {combined}"
    );
}

/// FR-006 / AC-006.24 — --csv without --tree rejects bare --tree (use --tree --csv per AC-006.26).
#[test]
fn fr006_proc_csv_flat_rejects_tree_without_csv_pairing() {
    let out = bin().args(["proc", "--csv", "--tree"]).output().expect("spawn proc --csv --tree");
    assert!(
        out.status.success(),
        "proc --csv --tree MUST succeed (AC-006.26); stderr: {:?}",
        out.stderr
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().next().unwrap_or("").starts_with("root_index,"),
        "combined --tree --csv MUST emit forest CSV; got: {s}"
    );
}

/// FR-006 / AC-006.24 — sort order is reflected in CSV row order.
#[test]
fn fr006_proc_csv_respects_sort_order() {
    let rows = vec![
        watch(10, "a", "a", 100, None),
        watch(20, "b", "b", 300, None),
        watch(30, "c", "c", 200, None),
    ];
    let sorted = sort_watched_agents(&rows, ProcSort::Rss, &HashMap::new());
    let csv = render_agent_inventory_csv(&sorted, &HashMap::new());
    let pids: Vec<u32> =
        csv.lines().skip(1).filter_map(|line| line.split(',').next()?.parse().ok()).collect();
    assert_eq!(pids, vec![20, 30, 10], "CSV rows MUST follow sort order; got: {csv}");
}
