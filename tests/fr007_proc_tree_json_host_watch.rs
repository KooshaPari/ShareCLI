// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — host ResourceWatchSample on `sharecli proc --tree` JSON surfaces
//! FR: FR-007
//!
//! AC-007.15 `proc --tree --json` / `proc --tree --watch --json` emit host_watch

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.15); got: {host}");
    }
}

/// FR-007 / AC-007.15 — one-shot proc --tree --json carries live host resource watch.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_json_host_watch_shape() {
    let out = bin()
        .args(["proc", "--tree", "--json"])
        .output()
        .expect("spawn sharecli proc --tree --json");
    assert!(out.status.success(), "proc --tree --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --tree --json MUST emit valid JSON");
    let host =
        v.get("host_watch").expect("proc --tree --json MUST include host_watch object (AC-007.15)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.15 — tree NDJSON watch lines embed host_watch on every snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_watch_json_host_watch_shape() {
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
    let host = v.get("host_watch").expect("tree watch NDJSON MUST include host_watch (AC-007.15)");
    assert_host_watch_object(host);

    let _ = child.kill();
    let _ = child.wait();
}

/// FR-007 / AC-007.15 — serialized tree snapshot preserves host watch field names.
#[test]
fn fr007_proc_tree_json_host_watch_serializes_fields() {
    use sharecli::commands::proc::AgentTreeSnapshot;
    use sharecli::monitoring::HostResourceWatchJson;

    let snap = AgentTreeSnapshot {
        forests: vec![],
        roots: 0,
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
    let json = serde_json::to_string(&snap).expect("serialize tree snapshot");
    for key in HOST_WATCH_KEYS {
        assert!(json.contains(&format!("\"{key}\"")), "JSON MUST include {key}; got: {json}");
    }
}
