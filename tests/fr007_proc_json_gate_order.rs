// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate JSON key ordering on one-shot `sharecli proc` surfaces
//! FR: FR-007
//!
//! AC-007.24 `proc --json`, `proc --tree --json`, and `proc --pid --json` serialize
//! `"gate"` before `"host_watch"` in raw JSON (parity with watch NDJSON AC-007.22/23)

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.24)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.24)");
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.24); got: {raw}"
    );
}

/// FR-007 / AC-007.24 — one-shot proc --json preserves gate → host_watch key order.
#[test]
#[serial_test::serial]
fn fr007_proc_json_gate_ordering() {
    let out = bin().args(["proc", "--json"]).output().expect("spawn sharecli proc --json");
    assert!(out.status.success(), "proc --json MUST exit 0; stderr: {:?}", out.stderr);
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "proc --json");
}

/// FR-007 / AC-007.24 — one-shot proc --tree --json preserves gate → host_watch key order.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_json_gate_ordering() {
    let out = bin()
        .args(["proc", "--tree", "--json"])
        .output()
        .expect("spawn sharecli proc --tree --json");
    assert!(out.status.success(), "proc --tree --json MUST exit 0; stderr: {:?}", out.stderr);
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "proc --tree --json");
}

/// FR-007 / AC-007.24 — one-shot proc --pid --json preserves gate → host_watch key order.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_json_gate_ordering() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json"])
        .output()
        .expect("spawn sharecli proc --pid --json");
    assert!(out.status.success(), "proc --pid --json MUST exit 0; stderr: {:?}", out.stderr);
    let raw = String::from_utf8_lossy(&out.stdout);
    assert_json_gate_before_host_watch(&raw, "proc --pid --json");
}

/// FR-007 / AC-007.24 — serialized flat snapshot preserves gate → host_watch key order.
#[test]
fn fr007_proc_json_gate_order_serializes_fields() {
    use sharecli::commands::proc::AgentProcSnapshot;
    use sharecli::monitoring::HostResourceWatchJson;

    let snap = AgentProcSnapshot {
        agents: vec![],
        scanned: 0,
        watched: 0,
        gate: sharecli_fleet::GateStatusSnapshot {
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
        status: None,
    };
    let json = serde_json::to_string(&snap).expect("serialize proc snapshot");
    assert_json_gate_before_host_watch(&json, "AgentProcSnapshot");
}

/// FR-007 / AC-007.24 — serialized tree snapshot preserves gate → host_watch key order.
#[test]
fn fr007_proc_tree_json_gate_order_serializes_fields() {
    use sharecli::commands::proc::AgentTreeSnapshot;
    use sharecli::monitoring::HostResourceWatchJson;

    let snap = AgentTreeSnapshot {
        forests: vec![],
        roots: 0,
        gate: sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 3,
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
        pool: None,
        status: None,
    };
    let json = serde_json::to_string(&snap).expect("serialize tree snapshot");
    assert_json_gate_before_host_watch(&json, "AgentTreeSnapshot");
}

/// FR-007 / AC-007.24 — serialized pid detail snapshot preserves gate → host_watch key order.
#[test]
fn fr007_proc_pid_json_gate_order_serializes_fields() {
    use sharecli::commands::proc::ProcDetailSnapshot;
    use sharecli::monitoring::HostResourceWatchJson;

    let detail = ProcDetailSnapshot {
        pid: 42,
        ppid: 1,
        parent_comm: Some("init".into()),
        comm: "claude".into(),
        state: "S".into(),
        cmdline: vec!["claude".into()],
        family: Some("claude".into()),
        agent_ancestor: None,
        mem_rss_bytes: 4096,
        mem_rss: "4.0K".into(),
        fd_count: Some(7),
        gate: sharecli_fleet::GateStatusSnapshot {
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
        status: None,
    };
    let json = serde_json::to_string(&detail).expect("serialize proc detail snapshot");
    assert_json_gate_before_host_watch(&json, "ProcDetailSnapshot");
}
