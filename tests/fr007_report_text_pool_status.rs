//! FR-007 — `sharecli report` text pool + proc-scan operator sections (AC-007.74)
//! FR: FR-007
//!
//! `report` (text, one-shot + `--watch`) prints pool + proc-scan operator lines on stdout
//! after gate → host_watch (parity with AC-007.39 / AC-007.73 JSON key order).

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
const REPORT_HEADER: &str = "=== Fleet Analytics Report ===";
const POOL_PREFIX: &str = "Pool node";
const PROC_PREFIX: &str = "Proc scan";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print operator companions on stderr (AC-007.74); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_text_operator_order(stdout: &str, context: &str) {
    assert!(
        stdout.contains(REPORT_HEADER),
        "{context} MUST include report header (AC-007.74); got: {stdout}"
    );
    assert!(
        stdout.contains(GATE_MARKER),
        "{context} MUST include gate section (AC-007.74); got: {stdout}"
    );
    assert!(
        stdout.contains(WATCH_MARKER),
        "{context} MUST include host watch section (AC-007.74); got: {stdout}"
    );
    assert!(
        stdout.contains(POOL_PREFIX),
        "{context} MUST include pool operator line (AC-007.74); got: {stdout}"
    );
    assert!(
        stdout.contains(PROC_PREFIX),
        "{context} MUST include proc-scan operator line (AC-007.74); got: {stdout}"
    );

    let report_pos = stdout.find(REPORT_HEADER).expect("report header");
    let gate_pos = stdout.find(GATE_MARKER).expect("gate section");
    let watch_pos = stdout.find(WATCH_MARKER).expect("host watch section");
    let pool_pos = stdout.find(POOL_PREFIX).expect("pool operator line");
    let proc_pos = stdout.find(PROC_PREFIX).expect("proc-scan operator line");

    assert!(
        report_pos < gate_pos
            && gate_pos < watch_pos
            && watch_pos < pool_pos
            && pool_pos < proc_pos,
        "{context} MUST serialize report → gate → host_watch → pool → proc-scan (AC-007.74); got: {stdout}"
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
        "{context} MUST serialize gate → host_watch → pool → proc-scan (AC-007.74); got: {segment}"
    );
}

fn assert_text_watch_stdout(stdout: &str, context: &str) {
    let frame_count = stdout.matches(REPORT_HEADER).count();
    assert!(
        frame_count >= 2,
        "{context} MUST re-render at least twice in dwell window; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} stdout MUST include [watch] footer (AC-007.74); got: {stdout}"
    );
    for (idx, segment) in stdout.split(REPORT_HEADER).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) && segment.contains(POOL_PREFIX) {
            assert_frame_operator_order(segment, &format!("{context} frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.74 — one-shot report text prints pool + proc-scan after gate → host_watch.
#[test]
#[serial_test::serial]
fn fr007_report_text_pool_status_order() {
    let out = bin()
        .args(["report", "--format", "text"])
        .output()
        .expect("spawn sharecli report --format text");
    assert!(out.status.success(), "report text MUST exit 0; stderr: {:?}", out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_stderr_silent(&out.stderr, "report");
    assert_text_operator_order(&stdout, "report");
}

/// FR-007 / AC-007.74 — report --watch text keeps pool/proc-scan on stdout across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_report_watch_text_pool_status_order() {
    let mut child = bin()
        .args(["report", "--format", "text", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli report --format text --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, REPORT_HEADER, 2, Duration::from_millis(30_000));

    assert_stderr_silent(stderr.as_bytes(), "report --watch");
    assert_text_watch_stdout(&stdout, "report --watch");
}
