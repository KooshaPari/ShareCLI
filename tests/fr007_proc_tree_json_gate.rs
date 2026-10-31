// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate on `sharecli proc --tree` JSON surfaces
//! FR: FR-007
//!
//! AC-007.18 `proc --tree --json` / `proc --tree --watch --json` emit gate

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_KEYS: [&str; 5] = [
    "thermal_pressure",
    "detected_agents",
    "agent_total_rss_bytes",
    "agent_contention",
    "gate_decision",
];

fn assert_gate_object(gate: &serde_json::Value) {
    for key in GATE_KEYS {
        assert!(gate.get(key).is_some(), "gate MUST include {key} (AC-007.18); got: {gate}");
    }
}

/// FR-007 / AC-007.18 — one-shot proc --tree --json carries live thermal gate snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_json_gate_shape() {
    let out = bin()
        .args(["proc", "--tree", "--json"])
        .output()
        .expect("spawn sharecli proc --tree --json");
    assert!(out.status.success(), "proc --tree --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --tree --json MUST emit valid JSON");
    let gate = v.get("gate").expect("proc --tree --json MUST include gate object (AC-007.18)");
    assert_gate_object(gate);
}

/// FR-007 / AC-007.18 — tree NDJSON watch lines embed gate on every snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_watch_json_gate_shape() {
    let mut child = bin()
        .args(["proc", "--tree", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --tree --json --watch 1");

    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read first NDJSON line");
    assert!(!line.is_empty(), "tree watch --json MUST emit at least one NDJSON line");

    let v: serde_json::Value =
        serde_json::from_str(line.trim()).expect("tree watch NDJSON line MUST be valid JSON");
    assert!(v.get("ts").is_some(), "NDJSON line MUST include ts");
    let gate = v.get("gate").expect("tree watch NDJSON MUST include gate (AC-007.18)");
    assert_gate_object(gate);

    let _ = child.kill();
    let _ = child.wait();
}

/// FR-007 / AC-007.18 — serialized tree snapshot preserves gate field names.
#[test]
fn fr007_proc_tree_json_gate_serializes_fields() {
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
    for key in GATE_KEYS {
        assert!(json.contains(&format!("\"{key}\"")), "JSON MUST include {key}; got: {json}");
    }
}
