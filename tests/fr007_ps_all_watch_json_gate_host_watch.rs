//! FR-007 — `sharecli ps --all --watch --json` NDJSON gate + host_watch parity
//! FR: FR-007
//!
//! AC-007.49 `ps --all --watch --json` streams NDJSON with gate → host_watch on
//! every refresh; stderr carries text companions (parity with report watch AC-007.42).

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

fn assert_gate_before_watch(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    assert!(
        gate_pos < watch_pos,
        "{context} MUST print gate before host watch (AC-007.49); got: {segment}"
    );
}

fn assert_ndjson_gate_before_host_watch(line: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(line.trim()).expect("watch NDJSON line MUST be valid JSON");
    assert!(v.get("ts").is_some(), "{context} MUST include ts (AC-007.49)");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.49)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.49)");
    let gate_pos = line.find("\"gate\"").expect("gate key in NDJSON line");
    let host_pos = line.find("\"host_watch\"").expect("host_watch key in NDJSON line");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.49); got: {line}"
    );
    let host = v.get("host_watch").expect("host_watch object");
    for key in HOST_WATCH_KEYS {
        assert!(
            host.get(key).is_some(),
            "{context} host_watch MUST include {key} (AC-007.49); got: {host}"
        );
    }
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

/// FR-007 / AC-007.49 — watch NDJSON stderr carries gate before host_watch companions.
#[test]
#[serial_test::serial]
fn fr007_ps_all_watch_ndjson_stderr_gate_before_host_watch() {
    let mut child = bin()
        .args(["ps", "--all", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli ps --all --json --watch 1");

    let (_stdout, stderr) = drain_watch_pipes(&mut child, "
", 2, Duration::from_millis(30_000));

    assert!(
        stderr.contains(GATE_MARKER),
        "ps watch NDJSON stderr MUST include gate companion (AC-007.49); stderr: {stderr}"
    );
    assert!(
        stderr.contains(WATCH_MARKER),
        "ps watch NDJSON stderr MUST include host watch companion (AC-007.49); stderr: {stderr}"
    );
    assert_gate_before_watch(&stderr, "ps watch NDJSON stderr");
    assert!(
        stderr.contains("[watch]"),
        "ps watch NDJSON stderr MUST include [watch] footer (AC-007.49); stderr: {stderr}"
    );
}

/// FR-007 / AC-007.49 — watch NDJSON stdout stays pipe-clean.
#[test]
#[serial_test::serial]
fn fr007_ps_all_watch_ndjson_stdout_no_companion_leak() {
    let mut child = bin()
        .args(["ps", "--all", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli ps --all --json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, "
", 2, Duration::from_millis(30_000));

    assert!(
        !stdout.contains(GATE_MARKER),
        "ps NDJSON stdout MUST NOT leak gate companion (AC-007.49); got: {stdout}"
    );
    assert!(
        !stdout.contains(WATCH_MARKER),
        "ps NDJSON stdout MUST NOT leak host watch companion (AC-007.49); got: {stdout}"
    );
    assert!(
        !stdout.contains("[watch]"),
        "ps NDJSON stdout MUST NOT contain watch footer (AC-007.49); got: {stdout}"
    );
    assert!(
        !stdout.contains("\x1b[2J"),
        "ps NDJSON stdout MUST NOT contain terminal clear sequences (AC-007.49)"
    );

    for line in stdout.lines().filter(|l| !l.is_empty()) {
        let _: serde_json::Value =
            serde_json::from_str(line).expect("each ps NDJSON stdout line MUST parse");
    }
}

/// FR-007 / AC-007.49 — watch NDJSON lines embed gate before host_watch on every snapshot.
#[test]
#[serial_test::serial]
fn fr007_ps_all_watch_ndjson_gate_ordering() {
    let mut child = bin()
        .args(["ps", "--all", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli ps --all --json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, "
", 2, Duration::from_millis(30_000));

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "ps watch --json MUST emit at least two NDJSON lines in dwell window; got: {stdout}"
    );
    for (idx, line) in lines.iter().enumerate() {
        assert_ndjson_gate_before_host_watch(line, &format!("ps NDJSON line {}", idx + 1));
    }
}

/// FR-007 / AC-007.49 — ps --watch --json without --all fails loudly.
#[test]
#[serial_test::serial]
fn fr007_ps_watch_json_requires_all() {
    let out = bin()
        .args(["ps", "--json", "--watch", "1"])
        .output()
        .expect("spawn sharecli ps --json --watch 1");
    assert!(
        !out.status.success(),
        "ps --watch --json without --all MUST fail (AC-007.49); stdout: {:?}",
        out.stdout
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--all"),
        "ps --watch --json without --all MUST mention --all requirement (AC-007.49); stderr: {stderr}"
    );
}

/// FR-007 / AC-007.49 — serialized ps watch NDJSON envelope preserves gate → host_watch key order.
#[test]
fn fr007_ps_all_watch_ndjson_gate_order_serializes_fields() {
    use sharecli::commands::proc::AgentProcRow;
    use sharecli::commands::PsAllJson;
    use sharecli::commands::PsAllNdjsonLine;
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
    let line = PsAllNdjsonLine { ts: 1_700_000_000, snapshot: envelope };
    let json = serde_json::to_string(&line).expect("serialize ps watch NDJSON envelope");
    assert_ndjson_gate_before_host_watch(&json, "PsAllNdjsonLine");
}
