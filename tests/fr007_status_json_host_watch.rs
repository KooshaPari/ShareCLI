// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — host ResourceWatchSample on `sharecli status --json`
//! FR: FR-007
//!
//! AC-007.25 `status --json` emits top-level `host_watch` + `gate` siblings (proc parity)

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.25); got: {host}");
    }
}

/// FR-007 / AC-007.25 — status --json carries top-level host_watch sibling.
#[test]
#[serial_test::serial]
fn fr007_status_json_host_watch_shape() {
    let out = bin().args(["status", "--json"]).output().expect("spawn sharecli status --json");
    assert!(out.status.success(), "status --json MUST exit 0; stderr: {:?}", out.stderr);
    let raw = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("status --json MUST emit valid JSON");
    assert!(v.get("total_processes").is_some(), "status JSON MUST include total_processes");
    assert!(
        v.get("agents").and_then(|a| a.as_array()).is_some(),
        "status JSON agents MUST be a flat array (AC-007.25); got: {v}"
    );
    assert!(
        v.get("scanned").is_some() && v.get("watched").is_some(),
        "status JSON MUST include scanned + watched (AC-007.25)"
    );
    let host =
        v.get("host_watch").expect("status --json MUST include top-level host_watch (AC-007.25)");
    assert_host_watch_object(host);
    assert!(
        v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
        "status --json MUST include top-level gate (AC-007.25)"
    );
    assert!(
        !raw.contains("\"agents\":{\"agents\""),
        "status --json MUST NOT nest proc snapshot under agents (AC-007.25)"
    );
}

/// FR-007 / AC-007.25 — status --json preserves gate → host_watch key ordering.
#[test]
#[serial_test::serial]
fn fr007_status_json_gate_before_host_watch() {
    let out = bin().args(["status", "--json"]).output().expect("spawn sharecli status --json");
    assert!(out.status.success(), "status --json MUST exit 0");
    let raw = String::from_utf8_lossy(&out.stdout);
    let gate_pos = raw.find("\"gate\"").expect("gate key in status JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in status JSON");
    assert!(
        gate_pos < host_pos,
        "status --json MUST serialize gate before host_watch (AC-007.25); got: {raw}"
    );
}
