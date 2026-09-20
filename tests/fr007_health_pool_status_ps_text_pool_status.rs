//! FR-007 — health/pool/status/ps --all text pool + proc-scan operator sections (AC-007.76)
//! FR: FR-007
//!
//! `health`, `pool`, `status`, and `ps --all` (text, one-shot + `--watch`) print pool +
//! proc-scan operator lines on stdout after gate → host_watch (parity with AC-007.74/75).

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";
const POOL_PREFIX: &str = "Pool node";
const PROC_PREFIX: &str = "Proc scan";

const HEALTH_HEADER: &str = "Shared runtime health:";
const POOL_HEADER: &str = "=== Shared Runtime Pool Status ===";
const STATUS_HEADER: &str = "=== Process Status ===";
const PS_INVENTORY_HEADER: &str = "=== Host agents (proc scan) ===";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print operator companions on stderr (AC-007.76); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_text_operator_order(stdout: &str, body_header: &str, context: &str) {
    assert!(
        stdout.contains(body_header),
        "{context} MUST include body header (AC-007.76); got: {stdout}"
    );
    assert!(
        stdout.contains(GATE_MARKER),
        "{context} MUST include gate section (AC-007.76); got: {stdout}"
    );
    assert!(
        stdout.contains(WATCH_MARKER),
        "{context} MUST include host watch section (AC-007.76); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_PREFIX),
        "{context} MUST include pool operator line (AC-007.76); got: {stdout}"
    );
    assert!(
        stdout.contains(PROC_PREFIX),
        "{context} MUST include proc-scan operator line (AC-007.76); got: {stdout}"
    );

    let body_pos = stdout.find(body_header).expect("body header");
    let gate_pos = stdout.find(GATE_MARKER).expect("gate section");
    let watch_pos = stdout.find(WATCH_MARKER).expect("host watch section");
    let pool_pos = stdout.find(POOL_PREFIX).expect("pool operator line");
    let proc_pos = stdout.find(PROC_PREFIX).expect("proc-scan operator line");

    assert!(
        body_pos < gate_pos
            && gate_pos < watch_pos
            && watch_pos < pool_pos
            && pool_pos < proc_pos,
        "{context} MUST serialize body → gate → host_watch → pool → proc-scan (AC-007.76); got: {stdout}"
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

fn assert_frame_operator_order(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    let pool_pos = segment
        .find(POOL_PREFIX)
        .unwrap_or_else(|| panic!("{context} MUST include pool operator line; got: {segment}"));
    let proc_pos = segment.find(PROC_PREFIX).unwrap_or_else(|| {
        panic!("{context} MUST include proc-scan operator line; got: {segment}")
    });
    assert!(
        gate_pos < watch_pos && watch_pos < pool_pos && pool_pos < proc_pos,
        "{context} MUST serialize gate → host_watch → pool → proc-scan (AC-007.76); got: {segment}"
    );
}

fn assert_text_watch_stdout(stdout: &str, frame_header: &str, context: &str) {
    let frame_count = stdout.matches(frame_header).count();
    assert!(
        frame_count >= 2,
        "{context} MUST re-render at least twice in dwell window; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} stdout MUST include [watch] footer (AC-007.76); got: {stdout}"
    );
    for (idx, segment) in stdout.split(frame_header).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) && segment.contains(POOL_PREFIX) {
            assert_frame_operator_order(segment, &format!("{context} frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.76 — one-shot health text prints pool + proc-scan after gate → host_watch.
#[test]
#[serial_test::serial]
fn fr007_health_text_pool_status_order() {
    let out = bin().args(["health"]).output().expect("spawn sharecli health");
    assert!(out.status.success(), "health MUST exit 0; stderr: {:?}", out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_stderr_silent(&out.stderr, "health");
    assert_text_operator_order(&stdout, HEALTH_HEADER, "health");
}

/// FR-007 / AC-007.76 — health --watch text keeps pool/proc-scan on stdout across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_health_watch_text_pool_status_order() {
    let mut child = bin()
        .args(["health", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli health --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, HEALTH_HEADER, 2, Duration::from_millis(30_000));

    assert_stderr_silent(stderr.as_bytes(), "health --watch");
    assert_text_watch_stdout(&stdout, HEALTH_HEADER, "health --watch");
}

/// FR-007 / AC-007.76 — one-shot pool text prints pool + proc-scan after gate → host_watch.
#[test]
#[serial_test::serial]
fn fr007_pool_text_pool_status_order() {
    let out = bin().args(["pool"]).output().expect("spawn sharecli pool");
    assert!(out.status.success(), "pool MUST exit 0; stderr: {:?}", out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_stderr_silent(&out.stderr, "pool");
    assert_text_operator_order(&stdout, POOL_HEADER, "pool");
}

/// FR-007 / AC-007.76 — pool --watch text keeps pool/proc-scan on stdout across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_pool_watch_text_pool_status_order() {
    let mut child = bin()
        .args(["pool", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli pool --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, POOL_HEADER, 2, Duration::from_millis(30_000));

    assert_stderr_silent(stderr.as_bytes(), "pool --watch");
    assert_text_watch_stdout(&stdout, POOL_HEADER, "pool --watch");
}

/// FR-007 / AC-007.76 — one-shot status text prints pool + proc-scan after gate → host_watch.
#[test]
#[serial_test::serial]
fn fr007_status_text_pool_status_order() {
    let out = bin().args(["status"]).output().expect("spawn sharecli status");
    assert!(out.status.success(), "status MUST exit 0; stderr: {:?}", out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_stderr_silent(&out.stderr, "status");
    assert_text_operator_order(&stdout, STATUS_HEADER, "status");
}

/// FR-007 / AC-007.76 — status --watch text keeps pool/proc-scan on stdout across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_status_watch_text_pool_status_order() {
    let mut child = bin()
        .args(["status", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli status --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, STATUS_HEADER, 2, Duration::from_millis(30_000));

    assert_stderr_silent(stderr.as_bytes(), "status --watch");
    assert_text_watch_stdout(&stdout, STATUS_HEADER, "status --watch");
}

/// FR-007 / AC-007.76 — one-shot ps --all text prints pool + proc-scan after gate → host_watch.
#[test]
#[serial_test::serial]
fn fr007_ps_all_text_pool_status_order() {
    let out = bin().args(["ps", "--all"]).output().expect("spawn sharecli ps --all");
    assert!(out.status.success(), "ps --all MUST exit 0; stderr: {:?}", out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_stderr_silent(&out.stderr, "ps --all");
    assert_text_operator_order(&stdout, PS_INVENTORY_HEADER, "ps --all");
}

/// FR-007 / AC-007.76 — ps --all --watch text keeps pool/proc-scan on stdout across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_ps_all_watch_text_pool_status_order() {
    let mut child = bin()
        .args(["ps", "--all", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli ps --all --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, PS_INVENTORY_HEADER, 2, Duration::from_millis(30_000));

    assert_stderr_silent(stderr.as_bytes(), "ps --all --watch");
    assert_text_watch_stdout(&stdout, PS_INVENTORY_HEADER, "ps --all --watch");
}
