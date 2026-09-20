//! FR-007 — `sharecli report --format csv --watch`
//! FR: FR-007
//!
//! AC-007.90 CSV watch emits fleet body + gate → host_watch → pool → status companions
//! each tick on stdout; stderr silent on success.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const FRAME_MARKER: &str = "# sharecli-report-watch-frame";
const SUMMARY_CSV_HEADER: &str = "record,timestamp,uptime_seconds,total_processes,total_memory_mb,thermal_pressure,detected_agents,agent_contention,gate_decision";
const GATE_CSV_HEADER: &str =
    "record,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision";
const HOST_CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";
const POOL_CSV_HEADER: &str = "record,node_total,node_idle,bun_total,bun_idle,max_per_type,healthy";
const STATUS_CSV_HEADER: &str = "record,scanned,watched,total_processes,agent_rows";

fn drain_watch_pipes(child: &mut Child, dwell: Duration) -> (String, String) {
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_reader = thread::spawn(move || {
        let mut buf = String::new();
        let mut out = stdout;
        let _ = out.read_to_string(&mut buf);
        buf
    });
    let stderr_reader = thread::spawn(move || {
        let mut buf = String::new();
        let mut err = stderr;
        let _ = err.read_to_string(&mut buf);
        buf
    });
    thread::sleep(dwell);
    let _ = child.kill();
    let _ = child.wait();
    (stdout_reader.join().expect("stdout drain"), stderr_reader.join().expect("stderr drain"))
}

fn assert_csv_envelope(frame: &str, context: &str) {
    let body = frame
        .find(SUMMARY_CSV_HEADER)
        .unwrap_or_else(|| panic!("{context} MUST include fleet summary CSV header; got: {frame}"));
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
        "{context} MUST order body → gate → host_watch → pool → status (AC-007.90); got: {frame}"
    );
}

/// FR-007 / AC-007.90 — report --format csv --watch stderr silent; multi-frame envelope.
#[test]
#[serial_test::serial]
fn fr007_report_csv_watch_stderr_silent_and_envelope() {
    let mut child = bin()
        .args(["report", "--format", "csv", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn report --format csv --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, Duration::from_millis(18_000));

    assert!(
        stderr.is_empty(),
        "report --format csv --watch MUST keep stderr silent (AC-007.90); stderr: {stderr:?}"
    );
    assert!(
        !stdout.contains("\x1b[2J"),
        "report --format csv --watch MUST NOT emit ANSI clear (pipe-safe); got: {stdout}"
    );
    let complete_frames: Vec<&str> = stdout
        .split(FRAME_MARKER)
        .skip(1)
        .filter(|frame| frame.contains(SUMMARY_CSV_HEADER))
        .collect();
    assert!(
        complete_frames.len() >= 2,
        "report --format csv --watch MUST emit >=2 complete frames; got {} in: {stdout}",
        complete_frames.len()
    );
    assert!(
        stdout.contains("[watch]"),
        "report --format csv --watch MUST include [watch] footer comment; got: {stdout}"
    );
    for (idx, frame) in complete_frames.iter().enumerate() {
        assert_csv_envelope(frame, &format!("report csv watch frame {}", idx + 1));
    }
}

/// FR-007 / AC-007.90 — report --format csv --watch 0 remains rejected.
#[test]
fn fr007_report_csv_watch_zero_interval_rejected() {
    let out = bin()
        .args(["report", "--format", "csv", "--watch", "0"])
        .output()
        .expect("spawn report --format csv --watch 0");
    assert!(!out.status.success(), "report --format csv --watch 0 MUST fail (AC-007.90)");
}
