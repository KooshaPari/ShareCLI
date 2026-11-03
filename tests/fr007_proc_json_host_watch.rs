// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — host ResourceWatchSample on `sharecli proc` JSON surfaces
//! FR: FR-007
//!
//! AC-007.13 `proc --json` / `proc --watch --json` emit host watch fields

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.13); got: {host}");
    }
}

/// FR-007 / AC-007.13 — one-shot proc --json carries live host resource watch.
#[test]
#[serial_test::serial]
fn fr007_proc_json_host_watch_shape() {
    let out = bin().args(["proc", "--json"]).output().expect("spawn sharecli proc --json");
    assert!(out.status.success(), "proc --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --json MUST emit valid JSON");
    let host = v.get("host_watch").expect("proc --json MUST include host_watch object (AC-007.13)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.13 — NDJSON watch lines embed host_watch on every snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_json_host_watch_shape() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read first NDJSON line");
    assert!(!line.is_empty(), "watch --json MUST emit at least one NDJSON line");

    let v: serde_json::Value =
        serde_json::from_str(line.trim()).expect("watch NDJSON line MUST be valid JSON");
    assert!(v.get("ts").is_some(), "NDJSON line MUST include ts");
    let host = v.get("host_watch").expect("watch NDJSON MUST include host_watch (AC-007.13)");
    assert_host_watch_object(host);

    let _ = child.kill();
    let _ = child.wait();
}

/// FR-007 / AC-007.13 — serialized snapshot preserves host watch field names.
#[test]
fn fr007_proc_json_host_watch_serializes_fields() {
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
    for key in HOST_WATCH_KEYS {
        assert!(json.contains(&format!("\"{key}\"")), "JSON MUST include {key}; got: {json}");
    }
}
