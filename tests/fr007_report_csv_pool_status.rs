// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli report --format csv` pool + proc-scan operator companion rows (AC-007.81)
//! FR: FR-007
//!
//! `report --format csv` appends pool + status companion records after fleet analytics body
//! and gate → host_watch (parity with proc CSV AC-007.79 and report text AC-007.74).

use std::process::Command;

use sharecli::commands::report::{build_report, render_report_csv_body, SortBy};
use sharecli_fleet::{gate_status_snapshot, PoolOperatorPanel, StatusOperatorPanel, ThermalLevel};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const SUMMARY_CSV_HEADER: &str =
    "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_csv_pool_status_companion_order(stdout: &str, context: &str) {
    assert!(
        stdout.contains(GATE_CSV_HEADER),
        "{context} MUST include gate CSV header (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.contains(HOST_CSV_HEADER),
        "{context} MUST include host_watch CSV header (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_CSV_HEADER),
        "{context} MUST include pool CSV header (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.contains(STATUS_CSV_HEADER),
        "{context} MUST include status CSV header (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("gate,")),
        "{context} MUST include gate companion row (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("host,")),
        "{context} MUST include host companion row (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("pool,")),
        "{context} MUST include pool companion row (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("status,")),
        "{context} MUST include status companion row (AC-007.81); got: {stdout}"
    );

    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = stdout.find(HOST_CSV_HEADER).expect("host header");
    let pool_pos = stdout.find(POOL_CSV_HEADER).expect("pool header");
    let status_pos = stdout.find(STATUS_CSV_HEADER).expect("status header");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.81); got: {stdout}"
    );
}

fn assert_report_body_precedes_companions(stdout: &str, context: &str) {
    assert!(
        stdout.contains(SUMMARY_CSV_HEADER),
        "{context} MUST include fleet summary CSV header (AC-007.81); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("summary,")),
        "{context} MUST include fleet summary row (AC-007.81); got: {stdout}"
    );
    let summary_pos = stdout.find(SUMMARY_CSV_HEADER).expect("summary header");
    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    assert!(
        summary_pos < gate_pos,
        "{context} MUST serialize fleet body before gate companions (AC-007.81); got: {stdout}"
    );
}

/// FR-007 / AC-007.81 — unit helper renders fleet analytics CSV body.
#[test]
fn fr007_report_render_csv_body() {
    let gate = gate_status_snapshot(ThermalLevel::Green, 0);
    let report = build_report(&[], &gate, &SortBy::Memory);
    let csv = render_report_csv_body(&report);
    assert!(csv.contains(SUMMARY_CSV_HEADER), "CSV body MUST include summary header; got: {csv}");
    assert!(
        csv.lines().any(|line| line.starts_with("summary,")),
        "CSV body MUST include summary row; got: {csv}"
    );
}

/// FR-007 / AC-007.81 — CLI report --format csv appends pool + status companions after host_watch.
#[test]
#[serial_test::serial]
fn fr007_report_csv_pool_status_companion() {
    let out = bin()
        .args(["report", "--format", "csv"])
        .output()
        .expect("spawn sharecli report --format csv");
    assert!(out.status.success(), "report --format csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert_report_body_precedes_companions(&s, "report --format csv");
    assert_csv_pool_status_companion_order(&s, "report --format csv");
}

/// FR-007 / AC-007.81 — companion CSV pool/status helpers match proc CSV shapes (AC-007.79).
#[test]
fn fr007_report_csv_companion_helpers_match_proc_shapes() {
    let pool_csv = PoolOperatorPanel {
        node_total: 2,
        node_idle: 1,
        bun_total: 0,
        bun_idle: 0,
        max_per_type: 4,
        healthy: false,
    }
    .format_csv_companion();
    assert!(pool_csv.contains(POOL_CSV_HEADER));

    let status_csv =
        StatusOperatorPanel { scanned: 1, watched: 0, total_processes: 3, agent_rows: 0 }
            .format_csv_companion();
    assert!(status_csv.contains(STATUS_CSV_HEADER));
}
