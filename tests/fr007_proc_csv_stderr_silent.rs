// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — one-shot `sharecli proc` CSV stderr silence (inverse of AC-007.28/29)
//! FR: FR-007
//!
//! AC-007.33 `proc --csv` and `proc --tree --csv` (no `--watch`) MUST NOT print gate or
//! host_watch text companions on stderr; gate/host_watch stay in CSV companion rows on stdout
//! only (parity with AC-007.30 / AC-007.31 / AC-007.32; extends AC-007.19).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

const POOL_TEXT_PREFIX: &str = "Pool node";
const PROC_TEXT_PREFIX: &str = "Proc scan";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.33); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.33); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.33); stderr: {s}"
    );
}

fn assert_csv_body_has_gate_and_host_watch(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(GATE_CSV_HEADER),
        "{context} CSV body MUST include gate companion header (AC-007.33); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("gate,")),
        "{context} CSV body MUST include gate companion row (AC-007.33); got: {s}"
    );
    assert!(
        s.contains(HOST_CSV_HEADER),
        "{context} CSV body MUST include host_watch companion header (AC-007.33); got: {s}"
    );
    let gate_pos = s.find(GATE_CSV_HEADER).expect("gate CSV header");
    let host_pos = s.find(HOST_CSV_HEADER).expect("host_watch CSV header");
    assert!(
        gate_pos < host_pos,
        "{context} gate companion MUST precede host_watch companion (AC-007.33); got: {s}"
    );
}

fn assert_csv_body_has_pool_and_status(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(POOL_CSV_HEADER),
        "{context} CSV body MUST include pool companion header (AC-007.79); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("pool,")),
        "{context} CSV body MUST include pool companion row (AC-007.79); got: {s}"
    );
    assert!(
        s.contains(STATUS_CSV_HEADER),
        "{context} CSV body MUST include status companion header (AC-007.79); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("status,")),
        "{context} CSV body MUST include status companion row (AC-007.79); got: {s}"
    );
    let host_pos = s.find(HOST_CSV_HEADER).expect("host_watch CSV header");
    let pool_pos = s.find(POOL_CSV_HEADER).expect("pool CSV header");
    let status_pos = s.find(STATUS_CSV_HEADER).expect("status CSV header");
    assert!(
        host_pos < pool_pos && pool_pos < status_pos,
        "{context} pool/status companions MUST follow host_watch (AC-007.79); got: {s}"
    );
}

fn assert_stderr_no_pool_status_text(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(POOL_TEXT_PREFIX),
        "{context} stderr MUST NOT include pool operator text (AC-007.79); stderr: {s}"
    );
    assert!(
        !s.contains(PROC_TEXT_PREFIX),
        "{context} stderr MUST NOT include proc-scan operator text (AC-007.79); stderr: {s}"
    );
}

/// FR-007 / AC-007.33 — one-shot proc --csv keeps stderr silent; gate/host_watch in CSV only.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_stderr_silent() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn sharecli proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "proc --csv");
    assert_stderr_no_companion_markers(&out.stderr, "proc --csv");
    assert_stderr_no_pool_status_text(&out.stderr, "proc --csv");
    assert_csv_body_has_gate_and_host_watch(&out.stdout, "proc --csv");
    assert_csv_body_has_pool_and_status(&out.stdout, "proc --csv");
}

/// FR-007 / AC-007.33 — one-shot proc --tree --csv keeps stderr silent; gate/host_watch in CSV only.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_csv_stderr_silent() {
    let out =
        bin().args(["proc", "--tree", "--csv"]).output().expect("spawn sharecli proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "proc --tree --csv");
    assert_stderr_no_companion_markers(&out.stderr, "proc --tree --csv");
    assert_stderr_no_pool_status_text(&out.stderr, "proc --tree --csv");
    assert_csv_body_has_gate_and_host_watch(&out.stdout, "proc --tree --csv");
    assert_csv_body_has_pool_and_status(&out.stdout, "proc --tree --csv");
}
