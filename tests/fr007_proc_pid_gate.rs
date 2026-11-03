// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate on `sharecli proc --pid` detail surfaces
//! FR: FR-007
//!
//! AC-007.17 `proc --pid N --json` emits gate; text detail prints gate section

use std::process::Command;

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
        assert!(gate.get(key).is_some(), "gate MUST include {key} (AC-007.17); got: {gate}");
    }
}

/// FR-007 / AC-007.17 — one-shot proc --pid --json carries live thermal gate snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_json_gate_shape() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json"])
        .output()
        .expect("spawn sharecli proc --pid --json");
    assert!(out.status.success(), "proc --pid --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --pid --json MUST emit valid JSON");
    let gate = v.get("gate").expect("proc --pid --json MUST include gate object (AC-007.17)");
    assert_gate_object(gate);
}

/// FR-007 / AC-007.17 — proc --pid text detail prints thermal gate section before host watch.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_text_gate_section() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string()])
        .output()
        .expect("spawn sharecli proc --pid");
    assert!(out.status.success(), "proc --pid MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("=== Thermal Gate (FR-011) ==="),
        "proc --pid text MUST include gate section (AC-007.17); got: {s}"
    );
    assert!(
        s.contains("Gate decision:"),
        "proc --pid text MUST include gate decision (AC-007.17); got: {s}"
    );
    let gate_pos = s.find("=== Thermal Gate (FR-011) ===").expect("gate section");
    let watch_pos = s.find("=== Host Resource Watch ===").expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "gate section MUST precede host watch footer (AC-007.17); got: {s}"
    );
}

/// FR-007 / AC-007.17 — serialized proc detail snapshot preserves gate field names.
#[test]
fn fr007_proc_pid_json_gate_serializes_fields() {
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
    let json = serde_json::to_string(&detail).expect("serialize proc detail snapshot");
    for key in GATE_KEYS {
        assert!(json.contains(&format!("\"{key}\"")), "JSON MUST include {key}; got: {json}");
    }
}
