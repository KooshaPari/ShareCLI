//! FR-007 — operator sibling `--csv --watch` (health/pool/status/ps --all)
//! FR: FR-007
//!
//! AC-007.89 CSV watch emits command body + gate → host_watch → pool → status companions
//! each tick on stdout; stderr silent on success.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const HEALTH_FRAME: &str = "# sharecli-health-watch-frame";
const POOL_FRAME: &str = "# sharecli-pool-watch-frame";
const STATUS_FRAME: &str = "# sharecli-status-watch-frame";
const PS_FRAME: &str = "# sharecli-ps-watch-frame";

const HEALTH_CSV_HEADER: &str =
    "record,healthy,node_total,node_idle,node_in_use,bun_total,bun_idle,bun_in_use,max_per_type,issues";
const POOL_BODY_CSV_HEADER: &str =
    "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy,issues";
const STATUS_SUMMARY_CSV_HEADER: &str = "record,total_processes,scanned,watched,agent_rows";
const PS_MANAGED_CSV_HEADER: &str = "record,pid,name,memory_mb,project,harness,agent";

const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";
const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";
const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";
const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

/// Drain both pipes until enough `frame_marker` lines have appeared, or `max` elapses.
///
/// A watch tick is scan-bound, not interval-bound: one iteration of `status --csv` measures
/// ~4 s of wall clock at near-zero CPU on a loaded host, so `--watch 1` does not mean a 1 s
/// cadence and a fixed sleep can capture a single frame. Waiting for the frames the assertion
/// actually needs is faster on an idle host (it returns as soon as they arrive) and correct on
/// a busy one. The marker precedes its frame body, so `frames_wanted + 1` occurrences are
/// needed before `frames_wanted` bodies are complete.
fn drain_watch_until(
    child: &mut Child,
    frame_marker: &str,
    frames_wanted: usize,
    max: Duration,
) -> (String, String) {
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let out_buf = Arc::new(Mutex::new(String::new()));
    let err_buf = Arc::new(Mutex::new(String::new()));

    let out_sink = Arc::clone(&out_buf);
    let stdout_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            out_sink.lock().unwrap().push_str(&line);
            line.clear();
        }
    });
    let err_sink = Arc::clone(&err_buf);
    let stderr_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            err_sink.lock().unwrap().push_str(&line);
            line.clear();
        }
    });

    let deadline = Instant::now() + max;
    // The marker is written before its frame body, so N complete frames need N+1 markers.
    // Stopping on the Nth marker would truncate the Nth frame and the envelope assertion
    // (which requires a complete body) would fail.
    let needed = frames_wanted + 1;
    loop {
        let seen = out_buf.lock().unwrap().matches(frame_marker).count();
        if seen >= needed || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(150));
    }

    let _ = child.kill();
    let _ = child.wait();
    stdout_reader.join().expect("stdout drain");
    stderr_reader.join().expect("stderr drain");

    let stdout_text = out_buf.lock().unwrap().clone();
    let stderr_text = err_buf.lock().unwrap().clone();
    (stdout_text, stderr_text)
}

fn assert_csv_envelope(frame: &str, body_header: &str, context: &str) {
    let body = frame
        .find(body_header)
        .unwrap_or_else(|| panic!("{context} MUST include body CSV header; got: {frame}"));
    let gate = frame[body..]
        .find(GATE_CSV_HEADER)
        .map(|p| body + p)
        .unwrap_or_else(|| panic!("{context} MUST include gate CSV companion; got: {frame}"));
    let host = frame[gate..]
        .find(HOST_CSV_HEADER)
        .map(|p| gate + p)
        .unwrap_or_else(|| panic!("{context} MUST include host_watch CSV companion; got: {frame}"));
    let pool = frame[host..]
        .find(POOL_CSV_HEADER)
        .map(|p| host + p)
        .unwrap_or_else(|| panic!("{context} MUST include pool CSV companion; got: {frame}"));
    let status = frame[pool..]
        .find(STATUS_CSV_HEADER)
        .map(|p| pool + p)
        .unwrap_or_else(|| panic!("{context} MUST include status CSV companion; got: {frame}"));
    assert!(
        body < gate && gate < host && host < pool && pool < status,
        "{context} MUST order body → gate → host_watch → pool → status (AC-007.89); got: {frame}"
    );
}

fn assert_csv_watch_contract(args: &[&str], frame_marker: &str, body_header: &str, context: &str) {
    let mut child = bin()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {context}: {e}"));

    let (stdout, stderr) = drain_watch_until(&mut child, frame_marker, 2, Duration::from_secs(60));

    assert!(stderr.is_empty(), "{context} MUST keep stderr silent (AC-007.89); stderr: {stderr:?}");
    assert!(
        !stdout.contains("\x1b[2J"),
        "{context} MUST NOT emit ANSI clear (pipe-safe); got: {stdout}"
    );
    let complete_frames: Vec<&str> =
        stdout.split(frame_marker).skip(1).filter(|frame| frame.contains(body_header)).collect();
    assert!(
        complete_frames.len() >= 2,
        "{context} MUST emit >=2 complete frames; got {} in: {stdout}",
        complete_frames.len()
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} MUST include [watch] footer comment; got: {stdout}"
    );
    for (idx, frame) in complete_frames.iter().enumerate() {
        assert_csv_envelope(frame, body_header, &format!("{context} frame {}", idx + 1));
    }
}

/// FR-007 / AC-007.89 — health --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_health_csv_watch_stderr_silent_and_envelope() {
    assert_csv_watch_contract(
        &["health", "--csv", "--watch", "1"],
        HEALTH_FRAME,
        HEALTH_CSV_HEADER,
        "health --csv --watch",
    );
}

/// FR-007 / AC-007.89 — pool --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_pool_csv_watch_stderr_silent_and_envelope() {
    assert_csv_watch_contract(
        &["pool", "--csv", "--watch", "1"],
        POOL_FRAME,
        POOL_BODY_CSV_HEADER,
        "pool --csv --watch",
    );
}

/// FR-007 / AC-007.89 — status --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_status_csv_watch_stderr_silent_and_envelope() {
    assert_csv_watch_contract(
        &["status", "--csv", "--watch", "1"],
        STATUS_FRAME,
        STATUS_SUMMARY_CSV_HEADER,
        "status --csv --watch",
    );
}

/// FR-007 / AC-007.89 — ps --all --csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_ps_all_csv_watch_stderr_silent_and_envelope() {
    assert_csv_watch_contract(
        &["ps", "--all", "--csv", "--watch", "1"],
        PS_FRAME,
        PS_MANAGED_CSV_HEADER,
        "ps --all --csv --watch",
    );
}

/// FR-007 / AC-007.89 — health --csv --json --watch remains rejected.
#[test]
fn fr007_health_csv_json_watch_still_rejected() {
    let out = bin()
        .args(["health", "--csv", "--json", "--watch", "1"])
        .output()
        .expect("spawn health --csv --json --watch");
    assert!(!out.status.success(), "health --csv --json --watch MUST fail (AC-007.89)");
}

/// FR-007 / AC-007.89 — ps --csv --watch without --all remains rejected.
#[test]
fn fr007_ps_csv_watch_without_all_still_rejected() {
    let out = bin().args(["ps", "--csv", "--watch", "1"]).output().expect("spawn ps --csv --watch");
    assert!(!out.status.success(), "ps --csv --watch without --all MUST fail (AC-007.89)");
    let combined =
        format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    assert!(
        combined.contains("--all") || combined.contains("AC-007.83"),
        "MUST require --all for ps --csv --watch; got: {combined}"
    );
}
