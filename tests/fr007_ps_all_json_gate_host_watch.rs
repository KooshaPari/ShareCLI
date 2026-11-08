// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — `sharecli ps --all --json` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.43 `ps --all --json` emits top-level `gate` + `host_watch` siblings after
//! managed pool + host agent inventory fields (parity with status --json AC-007.25 /
//! ps --all text AC-007.38); stderr silent on success.

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
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.43); got: {host}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.43)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.43)");
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.43); got: {raw}"
    );
}

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.43); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.43); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.43); stderr: {s}"
    );
}

/// FR-007 / AC-007.43 — ps --all --json carries managed pool + agent inventory + gate/host_watch.
#[test]
#[serial_test::serial]
fn fr007_ps_all_json_gate_host_watch_shape() {
    let out =
        bin().args(["ps", "--all", "--json"]).output().expect("spawn sharecli ps --all --json");
    assert!(out.status.success(), "ps --all --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("ps --all --json MUST emit valid JSON");
    assert!(
        v.get("processes").and_then(|p| p.as_array()).is_some(),
        "ps --all --json MUST include managed processes array (AC-007.43); got: {v}"
    );
    assert!(
        v.get("total_memory_mb").is_some(),
        "ps --all --json MUST include total_memory_mb (AC-007.43); got: {v}"
    );
    assert!(
        v.get("agents").and_then(|a| a.as_array()).is_some(),
        "ps --all --json agents MUST be a flat array (AC-007.43); got: {v}"
    );
    assert!(
        v.get("scanned").is_some() && v.get("watched").is_some(),
        "ps --all --json MUST include scanned + watched (AC-007.43)"
    );
    assert!(
        v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
        "ps --all --json MUST include top-level gate (AC-007.43); got: {v}"
    );
    let host =
        v.get("host_watch").expect("ps --all --json MUST include top-level host_watch (AC-007.43)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.43 — ps --all --json preserves gate → host_watch key ordering.
#[test]
#[serial_test::serial]
fn fr007_ps_all_json_gate_before_host_watch() {
    let out =
        bin().args(["ps", "--all", "--json"]).output().expect("spawn sharecli ps --all --json");
    assert!(out.status.success(), "ps --all --json MUST exit 0");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "ps --all --json");
}

/// FR-007 / AC-007.43 — one-shot ps --all --json keeps stderr silent; gate/host_watch in JSON only.
#[test]
#[serial_test::serial]
fn fr007_ps_all_json_stderr_silent() {
    let out =
        bin().args(["ps", "--all", "--json"]).output().expect("spawn sharecli ps --all --json");
    assert!(out.status.success(), "ps --all --json MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "ps --all --json");
    assert_stderr_no_companion_markers(&out.stderr, "ps --all --json");
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "ps --all --json");
}

/// FR-007 / AC-007.43 — ps --json without --all fails loudly.
#[test]
#[serial_test::serial]
fn fr007_ps_json_requires_all() {
    let out = bin().args(["ps", "--json"]).output().expect("spawn sharecli ps --json");
    assert!(
        !out.status.success(),
        "ps --json without --all MUST fail (AC-007.43); stdout: {:?}",
        out.stdout
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--all"),
        "ps --json without --all MUST mention --all requirement (AC-007.43); stderr: {stderr}"
    );
}

/// FR-007 / AC-007.43 — serialized ps --all JSON envelope preserves gate → host_watch key order.
#[test]
fn fr007_ps_all_json_gate_order_serializes_fields() {
    use sharecli::commands::proc::AgentProcRow;
    use sharecli::commands::PsAllJson;
    use sharecli::commands::PsManagedProcessRow;
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;

    let envelope = PsAllJson {
        processes: vec![PsManagedProcessRow {
            pid: 1,
            name: "test".into(),
            memory_mb: 10,
            project: Some("demo".into()),
            harness: Some("node".into()),
            agent: "-".into(),
        }],
        total_memory_mb: 10,
        agents: vec![AgentProcRow {
            pid: 42,
            family: "claude".into(),
            comm: "claude".into(),
            state: "S".into(),
            mem_rss_bytes: 1024,
            mem_rss: "1.0K".into(),
            fd_count: Some(8),
        }],
        scanned: 1,
        watched: 1,
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 1,
            agent_total_rss_bytes: 1024,
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
    let json = serde_json::to_string(&envelope).expect("serialize ps --all JSON envelope");
    assert_json_gate_before_host_watch(&json, "PsAllJson");
}
