// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli proc` CSV pool + proc-scan operator companion rows (AC-007.79)
//! FR: FR-007
//!
//! `proc --csv` and `proc --tree --csv` append pool + status companion records after
//! gate → host_watch (parity with text AC-007.75 and JSON AC-007.77).

use std::process::Command;

use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_csv_pool_status_companion_order(stdout: &str, context: &str) {
    assert!(
        stdout.contains(GATE_CSV_HEADER),
        "{context} MUST include gate CSV header (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.contains(HOST_CSV_HEADER),
        "{context} MUST include host_watch CSV header (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_CSV_HEADER),
        "{context} MUST include pool CSV header (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.contains(STATUS_CSV_HEADER),
        "{context} MUST include status CSV header (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("gate,")),
        "{context} MUST include gate companion row (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("host,")),
        "{context} MUST include host companion row (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("pool,")),
        "{context} MUST include pool companion row (AC-007.79); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("status,")),
        "{context} MUST include status companion row (AC-007.79); got: {stdout}"
    );

    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = stdout.find(HOST_CSV_HEADER).expect("host header");
    let pool_pos = stdout.find(POOL_CSV_HEADER).expect("pool header");
    let status_pos = stdout.find(STATUS_CSV_HEADER).expect("status header");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.79); got: {stdout}"
    );
}

/// FR-007 / AC-007.79 — unit helper renders companion CSV pool record.
#[test]
fn fr007_pool_format_csv_companion() {
    let csv = PoolOperatorPanel {
        node_total: 2,
        node_idle: 1,
        bun_total: 0,
        bun_idle: 0,
        max_per_type: 4,
        healthy: false,
    }
    .format_csv_companion();
    assert!(csv.contains(POOL_CSV_HEADER), "CSV companion MUST include pool header; got: {csv}");
    assert!(
        csv.trim().ends_with("pool,2,1,0,0,4,false"),
        "CSV companion MUST include pool data row; got: {csv}"
    );
}

/// FR-007 / AC-007.79 — unit helper renders companion CSV status record.
#[test]
fn fr007_status_format_csv_companion() {
    let csv = StatusOperatorPanel { scanned: 1, watched: 0, total_processes: 3, agent_rows: 0 }
        .format_csv_companion();
    assert!(
        csv.contains(STATUS_CSV_HEADER),
        "CSV companion MUST include status header; got: {csv}"
    );
    assert!(
        csv.trim().ends_with("status,1,0,3,0"),
        "CSV companion MUST include status data row; got: {csv}"
    );
}

/// FR-007 / AC-007.79 — CLI proc --csv appends pool + status companions after host_watch.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_pool_status_companion() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn sharecli proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().any(|line| line == "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"),
        "proc --csv MUST preserve agent header (AC-006.24); got: {s}"
    );
    assert_csv_pool_status_companion_order(&s, "proc --csv");
}

/// FR-007 / AC-007.79 — CLI proc --tree --csv appends pool + status companions after host_watch.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_csv_pool_status_companion() {
    let out =
        bin().args(["proc", "--tree", "--csv"]).output().expect("spawn sharecli proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().next().unwrap_or("").starts_with("root_index,"),
        "proc --tree --csv MUST preserve tree header; got: {s}"
    );
    assert_csv_pool_status_companion_order(&s, "proc --tree --csv");
}
