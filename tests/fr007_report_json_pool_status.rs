//! FR-007 — `sharecli report --format json` embedded pool + status siblings (AC-007.73)
//! FR: FR-007
//!
//! `report --format json` / `FleetReportJson` embed top-level `pool` + `status` after
//! `gate` → `host_watch` (parity with `monitoring.report` AC-007.72 / dashboard WS AC-007.70).

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

fn assert_json_gate_host_watch_pool_status_order(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.73)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.73)");
    let pool = v.get("pool").expect("{context} MUST include pool (AC-007.73)");
    let status = v.get("status").expect("{context} MUST include status (AC-007.73)");
    assert!(
        pool.get("node_total").is_some() && pool.get("healthy").is_some(),
        "pool MUST include capacity fields (AC-007.73); got: {pool}"
    );
    assert!(
        status.get("total_processes").is_some()
            && status.get("scanned").is_some()
            && status.get("watched").is_some(),
        "status MUST include proc-scan fields (AC-007.73); got: {status}"
    );
    assert!(gate.get("gate_decision").is_some(), "gate MUST include gate_decision (AC-007.73)");
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.73); got: {host}");
    }

    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let pool_pos = raw.find("\"pool\"").expect("pool key in raw JSON (AC-007.73)");
    let status_pos = raw.find("\"status\"").expect("status key in raw JSON (AC-007.73)");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.73); got: {raw}"
    );
}

/// Drain stdout/stderr while polling for `frames_needed` occurrences of
/// `frame_marker` in stdout. `max` bounds the total wait; if the watcher
/// can't emit that many frames in `max`, the call returns whatever was
/// captured so far (and the assertion below will likely fail).
///
/// Replaces a fixed-sleep drain that was flaky under parallel CI load: a
/// fixed sleep can land before the watcher has emitted enough frames.
fn drain_watch_pipes(
    child: &mut Child,
    frame_marker: &str,
    frames_needed: usize,
    max: Duration,
) -> (String, String) {
    let stdout_handle = child.stdout.take().expect("piped stdout");
    let stderr_handle = child.stderr.take().expect("piped stderr");
    let stdout_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let out_reader = {
        let buf = stdout_buf.clone();
        thread::spawn(move || {
            let mut s = String::new();
            let mut out = stdout_handle;
            let _ = out.read_to_string(&mut s);
            *buf.lock().expect("stdout lock") = s;
        })
    };
    let err_reader = {
        let buf = stderr_buf.clone();
        thread::spawn(move || {
            let mut s = String::new();
            let mut err = stderr_handle;
            let _ = err.read_to_string(&mut s);
            *buf.lock().expect("stderr lock") = s;
        })
    };

    let deadline = Instant::now() + max;
    loop {
        let count = stdout_buf
            .lock()
            .expect("stdout lock")
            .matches(frame_marker)
            .count();
        if count >= frames_needed || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = out_reader.join();
    let _ = err_reader.join();
    let stdout = stdout_buf.lock().expect("stdout lock").clone();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();
    (stdout, stderr)
}

/// FR-007 / AC-007.73 — report --format json carries top-level pool + status siblings.
#[test]
#[serial_test::serial]
fn fr007_report_json_pool_status_shape() {
    let out = bin()
        .args(["report", "--format", "json"])
        .output()
        .expect("spawn sharecli report --format json");
    assert!(out.status.success(), "report --format json MUST exit 0; stderr: {:?}", out.stderr);
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("report --format json MUST emit valid JSON");
    assert!(
        v.get("timestamp").is_some() && v.get("total_processes").is_some(),
        "report JSON MUST include fleet analytics fields (AC-007.73); got: {v}"
    );
    assert_json_gate_host_watch_pool_status_order(
        &String::from_utf8_lossy(&out.stdout),
        "report --format json",
    );
}

/// FR-007 / AC-007.73 — one-shot report --format json keeps stderr silent.
#[test]
#[serial_test::serial]
fn fr007_report_json_pool_status_stderr_silent() {
    let out = bin()
        .args(["report", "--format", "json"])
        .output()
        .expect("spawn sharecli report --format json");
    assert!(out.status.success(), "report --format json MUST exit 0; stderr: {:?}", out.stderr);
    assert!(
        out.stderr.is_empty(),
        "report --format json MUST NOT print companions on stderr (AC-007.73); stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains(GATE_MARKER),
        "report --format json stderr MUST NOT include gate companion (AC-007.73)"
    );
    assert!(
        !stderr.contains(WATCH_MARKER),
        "report --format json stderr MUST NOT include host watch companion (AC-007.73)"
    );
}

/// FR-007 / AC-007.73 — serialized FleetReportJson preserves operator key order.
#[test]
fn fr007_report_json_pool_status_serializes_fields() {
    use std::collections::HashMap;

    use sharecli::commands::report::FleetReportJson;
    use sharecli::commands::{PoolJson, StatusJson};
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;

    let gate = GateStatusSnapshot {
        thermal_pressure: "GREEN".into(),
        detected_agents: 0,
        agent_total_rss_bytes: 0,
        agent_contention: "OK".into(),
        gate_decision: "ADMIT".into(),
    };
    let host_watch = HostResourceWatchJson {
        fd_count: 7,
        net_rx_bytes: 1024,
        net_tx_bytes: 2048,
        mem_rss_bytes: 4096,
        load_1m: 1.25,
    };
    let pool = PoolJson {
        node_total: 2,
        node_idle: 1,
        bun_total: 1,
        bun_idle: 0,
        max_per_type: 4,
        healthy: true,
        issues: vec![],
        gate: gate.clone(),
        host_watch,
        status: None,
    };
    let status = StatusJson {
        total_processes: 1,
        agents: vec![],
        scanned: 50,
        watched: 1,
        gate: gate.clone(),
        host_watch,
        pool: None,
        log_location: None,
    };
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
        gate,
        host_watch,
        pool,
        status,
    };
    let json = serde_json::to_string(&envelope).expect("serialize FleetReportJson");
    assert_json_gate_host_watch_pool_status_order(&json, "FleetReportJson");
}

/// FR-007 / AC-007.73 — watch NDJSON lines embed gate → host_watch → pool → status per refresh.
#[test]
#[serial_test::serial]
fn fr007_report_watch_ndjson_pool_status_ordering() {
    let mut child = bin()
        .args(["report", "--format", "json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli report --format json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, "
", 2, Duration::from_millis(30_000));

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "report watch --json MUST emit at least two NDJSON lines; got: {stdout}"
    );
    for (idx, line) in lines.iter().enumerate() {
        assert_json_gate_host_watch_pool_status_order(
            line,
            &format!("report NDJSON line {}", idx + 1),
        );
    }
}
