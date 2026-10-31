// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate on `sharecli proc` CSV surfaces
//! FR: FR-007
//!
//! AC-007.19 `proc --csv` and `proc --tree --csv` append gate companion records
//! (parity with text gate section and JSON `gate` from AC-006.13 / AC-007.18)

use std::process::Command;

use sharecli_fleet::{gate_status_snapshot_with_rss, GateStatusSnapshot, ThermalLevel};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

/// FR-007 / AC-007.19 — unit helper renders companion CSV gate record.
#[test]
fn fr007_gate_format_csv_companion() {
    let csv =
        gate_status_snapshot_with_rss(ThermalLevel::Yellow, 4, 52_428_800).format_csv_companion();
    assert!(csv.contains(GATE_CSV_HEADER), "CSV companion MUST include gate header; got: {csv}");
    assert!(
        csv.contains("gate,YELLOW,4,52428800,WARN,"),
        "CSV companion MUST include gate data row; got: {csv}"
    );
}

/// FR-007 / AC-007.19 — companion row preserves explicit snapshot fields.
#[test]
fn fr007_gate_format_csv_companion_explicit_snapshot() {
    let snap = GateStatusSnapshot {
        thermal_pressure: "GREEN".into(),
        detected_agents: 2,
        agent_total_rss_bytes: 4096,
        agent_contention: "OK".into(),
        gate_decision: "ADMIT".into(),
    };
    let csv = snap.format_csv_companion();
    assert!(
        csv.trim().ends_with("gate,GREEN,2,4096,OK,ADMIT"),
        "CSV companion MUST include gate data row; got: {csv}"
    );
}

/// FR-007 / AC-007.19 — CLI proc --csv appends companion gate record before host_watch.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_gate_companion() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn sharecli proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().any(|line| line == "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"),
        "proc --csv MUST preserve agent header (AC-006.24); got: {s}"
    );
    assert!(
        s.contains(GATE_CSV_HEADER),
        "proc --csv MUST include gate CSV header (AC-007.19); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("gate,")),
        "proc --csv MUST include gate companion row (AC-007.19); got: {s}"
    );
    assert!(
        s.contains(HOST_CSV_HEADER),
        "proc --csv MUST still include host_watch CSV header (AC-007.14); got: {s}"
    );
    let gate_pos = s.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = s.find(HOST_CSV_HEADER).expect("host header");
    assert!(
        gate_pos < host_pos,
        "gate companion MUST precede host_watch companion (AC-007.19); got: {s}"
    );
}

/// FR-007 / AC-007.19 — CLI proc --tree --csv appends companion gate record before host_watch.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_csv_gate_companion() {
    let out =
        bin().args(["proc", "--tree", "--csv"]).output().expect("spawn sharecli proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().next().unwrap_or("").starts_with("root_index,"),
        "proc --tree --csv MUST preserve tree header; got: {s}"
    );
    assert!(
        s.contains(GATE_CSV_HEADER),
        "proc --tree --csv MUST include gate CSV header (AC-007.19); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("gate,")),
        "proc --tree --csv MUST include gate companion row (AC-007.19); got: {s}"
    );
    let gate_pos = s.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = s.find(HOST_CSV_HEADER).expect("host header");
    assert!(
        gate_pos < host_pos,
        "gate companion MUST precede host_watch companion (AC-007.19); got: {s}"
    );
}
