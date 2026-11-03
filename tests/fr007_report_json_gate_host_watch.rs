// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli report --format json` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.40 `report --format json` emits top-level `gate` + `host_watch` siblings
//! (parity with status --json AC-007.25 / proc JSON AC-007.24 key order gate before host_watch);
//! stderr silent on success.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.40); got: {host}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.40)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.40)");
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.40); got: {raw}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.40); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.40); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.40); stderr: {s}"
    );
}

/// FR-007 / AC-007.40 — report --format json carries top-level gate + host_watch siblings.
#[test]
#[serial_test::serial]
fn fr007_report_json_gate_host_watch_shape() {
    let out = bin()
        .args(["report", "--format", "json"])
        .output()
        .expect("spawn sharecli report --format json");
    assert!(out.status.success(), "report --format json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("report --format json MUST emit valid JSON");
    assert!(
        v.get("timestamp").is_some() && v.get("total_processes").is_some(),
        "report JSON MUST include fleet analytics fields (AC-007.40); got: {v}"
    );
    assert!(
        v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
        "report --format json MUST include top-level gate (AC-007.40); got: {v}"
    );
    let host = v
        .get("host_watch")
        .expect("report --format json MUST include top-level host_watch (AC-007.40)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.40 — report --format json preserves gate → host_watch key ordering.
#[test]
#[serial_test::serial]
fn fr007_report_json_gate_before_host_watch() {
    let out = bin()
        .args(["report", "--format", "json"])
        .output()
        .expect("spawn sharecli report --format json");
    assert!(out.status.success(), "report --format json MUST exit 0");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "report --format json");
}

/// FR-007 / AC-007.40 — one-shot report --format json keeps stderr silent; gate/host_watch in JSON only.
#[test]
#[serial_test::serial]
fn fr007_report_json_stderr_silent() {
    let out = bin()
        .args(["report", "--format", "json"])
        .output()
        .expect("spawn sharecli report --format json");
    assert!(out.status.success(), "report --format json MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "report --format json");
    assert_stderr_no_companion_markers(&out.stderr, "report --format json");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "report --format json");
}

/// FR-007 / AC-007.40 — serialized report JSON envelope preserves gate → host_watch key order.
#[test]
fn fr007_report_json_gate_order_serializes_fields() {
    use std::collections::HashMap;

    use sharecli::commands::report::FleetReportJson;
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;

    let envelope = FleetReportJson {
        timestamp: 1,
        uptime_seconds: 2,
        total_processes: 0,
        total_memory_mb: 0,
        by_project: HashMap::new(),
        top_consumers: vec![],
        thermal_pressure: "GREEN".into(),
        detected_agents: 0,
        agent_contention: "OK".into(),
        gate_decision: "ADMIT".into(),
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        },
        host_watch: HostResourceWatchJson {
            fd_count: 7,
            net_rx_bytes: 1024,
            net_tx_bytes: 2048,
            mem_rss_bytes: 4096,
            load_1m: 1.25,
        },
        pool: sharecli::commands::PoolJson {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
            issues: vec![],
            gate: GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson {
                fd_count: 7,
                net_rx_bytes: 1024,
                net_tx_bytes: 2048,
                mem_rss_bytes: 4096,
                load_1m: 1.25,
            },
            status: None,
        },
        status: sharecli::commands::StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 50,
            watched: 1,
            gate: GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson {
                fd_count: 7,
                net_rx_bytes: 1024,
                net_tx_bytes: 2048,
                mem_rss_bytes: 4096,
                load_1m: 1.25,
            },
            pool: None,
            log_location: None,
        },
    };
    let json = serde_json::to_string(&envelope).expect("serialize report JSON envelope");
    assert_json_gate_before_host_watch(&json, "FleetReportJson");
}
