// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli health --json` / `sharecli pool --json` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.44 `health --json` and `pool --json` emit top-level `gate` + `host_watch` siblings
//! (parity with status --json AC-007.25 / ps --all --json AC-007.43); stderr silent on success.

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
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.44); got: {host}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.44)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.44)");
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.44); got: {raw}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.44); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.44); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.44); stderr: {s}"
    );
}

/// FR-007 / AC-007.44 — health --json carries top-level gate + host_watch siblings.
#[test]
#[serial_test::serial]
fn fr007_health_json_gate_host_watch_shape() {
    let out = bin().args(["health", "--json"]).output().expect("spawn sharecli health --json");
    assert!(out.status.success(), "health --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("health --json MUST emit valid JSON");
    assert!(v.get("healthy").is_some(), "health --json MUST include healthy (AC-007.44); got: {v}");
    assert!(
        v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
        "health --json MUST include top-level gate (AC-007.44); got: {v}"
    );
    let host =
        v.get("host_watch").expect("health --json MUST include top-level host_watch (AC-007.44)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.44 — pool --json carries top-level gate + host_watch siblings.
#[test]
#[serial_test::serial]
fn fr007_pool_json_gate_host_watch_shape() {
    let out = bin().args(["pool", "--json"]).output().expect("spawn sharecli pool --json");
    assert!(out.status.success(), "pool --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("pool --json MUST emit valid JSON");
    assert!(
        v.get("node_total").is_some() && v.get("bun_total").is_some(),
        "pool --json MUST include pool status fields (AC-007.44); got: {v}"
    );
    assert!(
        v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
        "pool --json MUST include top-level gate (AC-007.44); got: {v}"
    );
    let host =
        v.get("host_watch").expect("pool --json MUST include top-level host_watch (AC-007.44)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.44 — health --json preserves gate → host_watch key ordering.
#[test]
#[serial_test::serial]
fn fr007_health_json_gate_before_host_watch() {
    let out = bin().args(["health", "--json"]).output().expect("spawn sharecli health --json");
    assert!(out.status.success(), "health --json MUST exit 0");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "health --json");
}

/// FR-007 / AC-007.44 — pool --json preserves gate → host_watch key ordering.
#[test]
#[serial_test::serial]
fn fr007_pool_json_gate_before_host_watch() {
    let out = bin().args(["pool", "--json"]).output().expect("spawn sharecli pool --json");
    assert!(out.status.success(), "pool --json MUST exit 0");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "pool --json");
}

/// FR-007 / AC-007.44 — one-shot health --json keeps stderr silent; gate/host_watch in JSON only.
#[test]
#[serial_test::serial]
fn fr007_health_json_stderr_silent() {
    let out = bin().args(["health", "--json"]).output().expect("spawn sharecli health --json");
    assert!(out.status.success(), "health --json MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "health --json");
    assert_stderr_no_companion_markers(&out.stderr, "health --json");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "health --json");
}

/// FR-007 / AC-007.44 — one-shot pool --json keeps stderr silent; gate/host_watch in JSON only.
#[test]
#[serial_test::serial]
fn fr007_pool_json_stderr_silent() {
    let out = bin().args(["pool", "--json"]).output().expect("spawn sharecli pool --json");
    assert!(out.status.success(), "pool --json MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "pool --json");
    assert_stderr_no_companion_markers(&out.stderr, "pool --json");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "pool --json");
}

/// FR-007 / AC-007.44 — serialized health JSON envelope preserves gate → host_watch key order.
#[test]
fn fr007_health_json_gate_order_serializes_fields() {
    use sharecli::commands::HealthJson;
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;

    let envelope = HealthJson {
        healthy: true,
        issues: vec![],
        node_total: 2,
        node_idle: 1,
        node_in_use: 1,
        bun_total: 1,
        bun_idle: 0,
        bun_in_use: 1,
        max_per_type: 4,
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
            node_total: 1,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 1,
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
                fd_count: 1,
                net_rx_bytes: 1,
                net_tx_bytes: 2,
                mem_rss_bytes: 3,
                load_1m: 0.5,
            },
            status: None,
        },
        status: sharecli::commands::StatusJson {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson {
                fd_count: 1,
                net_rx_bytes: 1,
                net_tx_bytes: 2,
                mem_rss_bytes: 3,
                load_1m: 0.5,
            },
            pool: None,
            log_location: None,
        },
    };
    let json = serde_json::to_string(&envelope).expect("serialize health JSON envelope");
    assert_json_gate_before_host_watch(&json, "HealthJson");
}

/// FR-007 / AC-007.44 — serialized pool JSON envelope preserves gate → host_watch key order.
#[test]
fn fr007_pool_json_gate_order_serializes_fields() {
    use sharecli::commands::PoolJson;
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;

    let envelope = PoolJson {
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
    };
    let json = serde_json::to_string(&envelope).expect("serialize pool JSON envelope");
    assert_json_gate_before_host_watch(&json, "PoolJson");
}
