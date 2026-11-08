// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — host ResourceWatchSample on `sharecli proc --pid` JSON surfaces
//! FR: FR-007
//!
//! AC-007.16 `proc --pid N --json` emits host_watch; text detail footer parity

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

const TEXT_MARKERS: [&str; 5] = ["Open FDs:", "RSS:", "Load (1m):", "Net RX:", "Net TX:"];

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.16); got: {host}");
    }
}

/// FR-007 / AC-007.16 — one-shot proc --pid --json carries live host resource watch.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_json_host_watch_shape() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json"])
        .output()
        .expect("spawn sharecli proc --pid --json");
    assert!(out.status.success(), "proc --pid --json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("proc --pid --json MUST emit valid JSON");
    let host =
        v.get("host_watch").expect("proc --pid --json MUST include host_watch object (AC-007.16)");
    assert_host_watch_object(host);
}

/// FR-007 / AC-007.16 — proc --pid text detail appends host watch footer.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_text_host_watch_footer() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string()])
        .output()
        .expect("spawn sharecli proc --pid");
    assert!(out.status.success(), "proc --pid MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("=== Host Resource Watch ==="),
        "proc --pid text MUST include host watch section (AC-007.16); got: {s}"
    );
    for marker in TEXT_MARKERS {
        assert!(s.contains(marker), "proc --pid text MUST include {marker} (AC-007.16)");
    }
}

/// FR-007 / AC-007.16 — serialized proc detail snapshot preserves host watch field names.
#[test]
fn fr007_proc_pid_json_host_watch_serializes_fields() {
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
    for key in HOST_WATCH_KEYS {
        assert!(json.contains(&format!("\"{key}\"")), "JSON MUST include {key}; got: {json}");
    }
}
