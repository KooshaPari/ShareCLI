// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli report --format csv` stderr silence (AC-007.81)
//! FR: FR-007
//!
//! One-shot `report --format csv` MUST NOT print gate/host_watch/pool/status text on stderr;
//! operator companions stay in CSV rows on stdout only (parity with proc CSV AC-007.33/79).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";
const POOL_PREFIX: &str = "Pool node";
const PROC_PREFIX: &str = "Proc scan";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print operator companions on stderr (AC-007.81); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.81); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.81); stderr: {s}"
    );
    assert!(
        !s.contains(POOL_PREFIX),
        "{context} stderr MUST NOT include pool operator text (AC-007.81); stderr: {s}"
    );
    assert!(
        !s.contains(PROC_PREFIX),
        "{context} stderr MUST NOT include proc-scan operator text (AC-007.81); stderr: {s}"
    );
}

fn assert_csv_body_has_operator_companions(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(GATE_CSV_HEADER),
        "{context} CSV body MUST include gate companion header (AC-007.81); got: {s}"
    );
    assert!(
        s.contains(HOST_CSV_HEADER),
        "{context} CSV body MUST include host_watch companion header (AC-007.81); got: {s}"
    );
    assert!(
        s.contains(POOL_CSV_HEADER),
        "{context} CSV body MUST include pool companion header (AC-007.81); got: {s}"
    );
    assert!(
        s.contains(STATUS_CSV_HEADER),
        "{context} CSV body MUST include status companion header (AC-007.81); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("pool,")),
        "{context} CSV body MUST include pool companion row (AC-007.81); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("status,")),
        "{context} CSV body MUST include status companion row (AC-007.81); got: {s}"
    );
}

/// FR-007 / AC-007.81 — one-shot report --format csv keeps stderr silent; companions in CSV only.
#[test]
#[serial_test::serial]
fn fr007_report_csv_stderr_silent() {
    let out = bin()
        .args(["report", "--format", "csv"])
        .output()
        .expect("spawn sharecli report --format csv");
    assert!(out.status.success(), "report --format csv MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "report --format csv");
    assert_stderr_no_companion_markers(&out.stderr, "report --format csv");
    assert_csv_body_has_operator_companions(&out.stdout, "report --format csv");
}
