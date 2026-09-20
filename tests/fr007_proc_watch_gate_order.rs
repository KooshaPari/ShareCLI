//! FR-007 — thermal gate ordering on `sharecli proc --watch` refresh surfaces
//! FR: FR-007
//!
//! AC-007.22 `proc --watch` text and NDJSON preserve gate → host_watch ordering
//! on every refresh (parity with one-shot text AC-007.21)

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
const INVENTORY_HEADER: &str = "=== Host agents (proc scan) ===";

fn assert_gate_before_watch(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    assert!(gate_pos < watch_pos, "{context} MUST print gate before host watch; got: {segment}");
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

/// FR-007 / AC-007.22 — watch text re-renders gate before host watch on each refresh.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_text_gate_ordering() {
    let mut child = bin()
        .args(["proc", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, INVENTORY_HEADER, 2, Duration::from_millis(30_000));

    let frame_count = stdout.matches(INVENTORY_HEADER).count();
    assert!(
        frame_count >= 2,
        "watch MUST re-render at least twice in ~30s; got {frame_count} frames in: {stdout}"
    );

    for (idx, segment) in stdout.split(INVENTORY_HEADER).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) {
            assert_gate_before_watch(segment, &format!("watch text frame {}", idx + 1));
        }
    }
    assert!(
        stdout.contains(WATCH_MARKER),
        "watch text MUST eventually render host watch footer; got: {stdout}"
    );
}

/// FR-007 / AC-007.22 — watch NDJSON lines embed gate before host_watch on every snapshot.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_ndjson_gate_ordering() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, INVENTORY_HEADER, 2, Duration::from_millis(30_000));

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "watch --json MUST emit at least two NDJSON lines in ~30s; got: {stdout}"
    );
    for (idx, line) in lines.iter().enumerate() {
        assert_ndjson_gate_before_host_watch(line, &format!("NDJSON line {}", idx + 1));
    }
}

fn assert_ndjson_gate_before_host_watch(line: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(line.trim()).expect("watch NDJSON line MUST be valid JSON");
    assert!(v.get("ts").is_some(), "{context} MUST include ts");
    assert!(v.get("gate").is_some(), "{context} MUST include gate (AC-007.22)");
    assert!(v.get("host_watch").is_some(), "{context} MUST include host_watch (AC-007.22)");
    let gate_pos = line.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = line.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.22); got: {line}"
    );
}
