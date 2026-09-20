//! FR-007 — `sharecli health --watch` text stderr silence (inverse of AC-007.64 JSON)
//! FR: FR-007
//!
//! AC-007.64 `health --watch` (text mode, no `--json`) MUST NOT print gate or host_watch
//! text companions on stderr during refresh cycles; gate/host_watch and `[watch]` footer stay
//! on stdout only (parity with AC-007.50 ps text watch stderr silence).

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";
const HEALTH_HEADER: &str = "Shared runtime health:";

/// Drain both pipes until enough `frame_marker` lines have appeared, or `max` elapses.
///
/// A watch tick is scan-bound, not interval-bound: one iteration measures seconds of wall
/// clock at near-zero CPU on a loaded host, so `--watch 1` is not a 1 s cadence and a fixed
/// sleep can capture a single frame. Waiting for the frames the assertion needs is faster on
/// an idle host (it returns as soon as they arrive) and correct on a busy one.
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
    loop {
        let seen = out_buf.lock().unwrap().matches(frame_marker).count();
        if seen >= frames_wanted || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(150));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let out = out_buf.lock().unwrap().clone();
    let err = err_buf.lock().unwrap().clone();
    (out, err)
}

fn assert_stderr_silent(stderr: &str, context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr during refresh (AC-007.64); stderr: {stderr:?}"
    );
}

fn assert_stderr_no_companion_markers(stderr: &str, context: &str) {
    assert!(
        !stderr.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.64); stderr: {stderr}"
    );
    assert!(
        !stderr.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.64); stderr: {stderr}"
    );
    assert!(
        !stderr.contains("[watch]"),
        "{context} stderr MUST NOT include [watch] footer (AC-007.64); stderr: {stderr}"
    );
}

fn assert_gate_before_watch(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    assert!(
        gate_pos < watch_pos,
        "{context} gate section MUST precede host watch footer (AC-007.64); got: {segment}"
    );
}

fn assert_text_watch_stdout(stdout: &str, context: &str) {
    let frame_count = stdout.matches(HEALTH_HEADER).count();
    assert!(
        frame_count >= 2,
        "{context} MUST re-render at least twice in dwell window; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains(GATE_MARKER),
        "{context} stdout MUST include gate section (AC-007.64); got: {stdout}"
    );
    assert!(
        stdout.contains(WATCH_MARKER),
        "{context} stdout MUST include host watch section (AC-007.64); got: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} stdout MUST include [watch] footer (AC-007.64); got: {stdout}"
    );
    for (idx, segment) in stdout.split(HEALTH_HEADER).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) {
            assert_gate_before_watch(segment, &format!("{context} frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.64 — health --watch text keeps stderr silent across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_health_watch_text_stderr_silent() {
    let mut child = bin()
        .args(["health", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli health --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, HEALTH_HEADER, 2, Duration::from_secs(60));

    assert_stderr_silent(&stderr, "health --watch");
    assert_stderr_no_companion_markers(&stderr, "health --watch");
    assert_text_watch_stdout(&stdout, "health --watch");
}
