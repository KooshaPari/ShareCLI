// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli health|pool|status --csv` operator companion rows (AC-007.82)
//! FR: FR-007
//!
//! health/pool/status --csv emit command-specific CSV bodies followed by companion
//! gate → host_watch → pool → status records (parity with proc CSV AC-007.79 and report CSV AC-007.81).

use std::process::Command;

use sharecli::commands::{render_health_csv_body, render_pool_csv_body, HealthJson, PoolJson};
use sharecli_fleet::{gate_status_snapshot, PoolOperatorPanel, StatusOperatorPanel, ThermalLevel};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HEALTH_CSV_HEADER: &str =
    "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues";

const POOL_BODY_CSV_HEADER: &str =
    "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues";

const STATUS_SUMMARY_CSV_HEADER: &str = "record,total_processes,scanned,watched,agent_rows";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";

const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

const POOL_COMPANION_CSV_HEADER: &str =
    "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";

const STATUS_COMPANION_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn assert_csv_companion_order(stdout: &str, context: &str) {
    assert!(
        stdout.contains(GATE_CSV_HEADER),
        "{context} MUST include gate CSV header (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.contains(HOST_CSV_HEADER),
        "{context} MUST include host_watch CSV header (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_COMPANION_CSV_HEADER),
        "{context} MUST include pool CSV header (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.contains(STATUS_COMPANION_CSV_HEADER),
        "{context} MUST include status CSV header (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("gate,")),
        "{context} MUST include gate companion row (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("host,")),
        "{context} MUST include host companion row (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("pool,")),
        "{context} MUST include pool companion row (AC-007.82); got: {stdout}"
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("status,")),
        "{context} MUST include status companion row (AC-007.82); got: {stdout}"
    );

    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    let host_pos = stdout[gate_pos..]
        .find(HOST_CSV_HEADER)
        .map(|p| gate_pos + p)
        .expect("host header after gate");
    let pool_pos = stdout[host_pos..]
        .find(POOL_COMPANION_CSV_HEADER)
        .map(|p| host_pos + p)
        .expect("pool companion header after host");
    let status_pos = stdout[pool_pos..]
        .find(STATUS_COMPANION_CSV_HEADER)
        .map(|p| pool_pos + p)
        .expect("status companion header after pool");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.82); got: {stdout}"
    );
}

fn assert_body_precedes_companions(stdout: &str, body_header: &str, context: &str) {
    let body_pos = stdout.find(body_header).expect("body header");
    let gate_pos = stdout.find(GATE_CSV_HEADER).expect("gate header");
    assert!(
        body_pos < gate_pos,
        "{context} MUST serialize command body before gate companions (AC-007.82); got: {stdout}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch text on stderr (AC-007.82); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

/// FR-007 / AC-007.82 — unit helper renders health CSV body.
#[test]
fn fr007_render_health_csv_body() {
    let gate = gate_status_snapshot(ThermalLevel::Green, 0);
    let host_watch = sharecli::monitoring::HostResourceWatchJson::default();
    let pool = PoolJson {
        node_total: 1,
        node_idle: 1,
        bun_total: 0,
        bun_idle: 0,
        max_per_type: 2,
        healthy: true,
        issues: vec![],
        gate: gate.clone(),
        host_watch,
        status: None,
    };
    let health = HealthJson {
        healthy: true,
        issues: vec![],
        node_total: 1,
        node_idle: 1,
        node_in_use: 0,
        bun_total: 0,
        bun_idle: 0,
        bun_in_use: 0,
        max_per_type: 2,
        gate,
        host_watch,
        pool,
        status: sharecli::commands::StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: gate_status_snapshot(ThermalLevel::Green, 0),
            host_watch: sharecli::monitoring::HostResourceWatchJson::default(),
            pool: None,
            log_location: None,
        },
    };
    let csv = render_health_csv_body(&health);
    assert!(csv.contains(HEALTH_CSV_HEADER), "health CSV body MUST include header; got: {csv}");
    assert!(
        csv.lines().any(|line| line.starts_with("health,")),
        "health CSV body MUST include data row; got: {csv}"
    );
}

/// FR-007 / AC-007.82 — unit helper renders pool CSV body.
#[test]
fn fr007_render_pool_csv_body() {
    let gate = gate_status_snapshot(ThermalLevel::Green, 0);
    let pool = PoolJson {
        node_total: 2,
        node_idle: 1,
        bun_total: 0,
        bun_idle: 0,
        max_per_type: 4,
        healthy: false,
        issues: vec!["idle mismatch".to_string()],
        gate,
        host_watch: sharecli::monitoring::HostResourceWatchJson::default(),
        status: None,
    };
    let csv = render_pool_csv_body(&pool);
    assert!(csv.contains(POOL_BODY_CSV_HEADER), "pool CSV body MUST include header; got: {csv}");
    assert!(
        csv.lines().any(|line| line.starts_with("pool,")),
        "pool CSV body MUST include data row; got: {csv}"
    );
}

/// FR-007 / AC-007.82 — CLI health --csv appends gate → host_watch → pool → status companions.
#[test]
#[serial_test::serial]
fn fr007_health_csv_pool_status_companion() {
    let out = bin().args(["health", "--csv"]).output().expect("spawn sharecli health --csv");
    assert!(out.status.success(), "health --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert_body_precedes_companions(&s, HEALTH_CSV_HEADER, "health --csv");
    assert_csv_companion_order(&s, "health --csv");
    assert_stderr_silent(&out.stderr, "health --csv");
}

/// FR-007 / AC-007.82 — CLI pool --csv appends gate → host_watch → pool → status companions.
#[test]
#[serial_test::serial]
fn fr007_pool_csv_pool_status_companion() {
    let out = bin().args(["pool", "--csv"]).output().expect("spawn sharecli pool --csv");
    assert!(out.status.success(), "pool --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert_body_precedes_companions(&s, POOL_BODY_CSV_HEADER, "pool --csv");
    assert_csv_companion_order(&s, "pool --csv");
    assert_stderr_silent(&out.stderr, "pool --csv");
}

/// FR-007 / AC-007.82 — CLI status --csv appends gate → host_watch → pool → status companions.
#[test]
#[serial_test::serial]
fn fr007_status_csv_pool_status_companion() {
    let out = bin().args(["status", "--csv"]).output().expect("spawn sharecli status --csv");
    assert!(out.status.success(), "status --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert_body_precedes_companions(&s, STATUS_SUMMARY_CSV_HEADER, "status --csv");
    assert_csv_companion_order(&s, "status --csv");
    assert_stderr_silent(&out.stderr, "status --csv");
}

/// FR-007 / AC-007.82 — pool --csv rejects --json.
#[test]
#[serial_test::serial]
fn fr007_pool_csv_rejects_json() {
    let out =
        bin().args(["pool", "--csv", "--json"]).output().expect("spawn sharecli pool --csv --json");
    assert!(!out.status.success(), "pool --csv --json MUST fail loudly (AC-007.82)");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("csv") && stderr.contains("json"),
        "error MUST mention csv/json incompatibility (AC-007.82); stderr: {stderr}"
    );
}

/// FR-007 / AC-007.82 — companion CSV pool/status helpers match proc CSV shapes (AC-007.79).
#[test]
fn fr007_health_pool_status_csv_companion_helpers_match_proc_shapes() {
    let pool_csv = PoolOperatorPanel {
        node_total: 2,
        node_idle: 1,
        bun_total: 0,
        bun_idle: 0,
        max_per_type: 4,
        healthy: false,
    }
    .format_csv_companion();
    assert!(pool_csv.contains(POOL_COMPANION_CSV_HEADER));

    let status_csv =
        StatusOperatorPanel { scanned: 1, watched: 0, total_processes: 3, agent_rows: 0 }
            .format_csv_companion();
    assert!(status_csv.contains(STATUS_COMPANION_CSV_HEADER));
}
