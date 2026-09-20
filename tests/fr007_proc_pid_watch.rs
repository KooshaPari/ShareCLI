//! FR-007 — `sharecli proc --pid N --watch` refresh surfaces
//! FR: FR-007
//!
//! AC-007.87 `proc --pid N --watch [secs]` text/NDJSON watch parity with flat `proc --watch`

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
const DETAIL_HEADER: &str = "=== Process detail (PID ";
const POOL_PREFIX: &str = "Pool node";
const PROC_PREFIX: &str = "Proc scan";

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

fn assert_gate_before_watch(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    assert!(gate_pos < watch_pos, "{context} MUST print gate before host watch; got: {segment}");
}

fn assert_text_envelope_order(stdout: &str, context: &str) {
    let body_pos = stdout.find(DETAIL_HEADER).expect("proc detail header");
    let gate_pos = stdout.find(GATE_MARKER).expect("gate section");
    let watch_pos = stdout.find(WATCH_MARKER).expect("host watch section");
    let pool_pos = stdout.find(POOL_PREFIX).expect("pool operator line");
    let proc_pos = stdout.find(PROC_PREFIX).expect("proc-scan operator line");
    assert!(
        body_pos < gate_pos
            && gate_pos < watch_pos
            && watch_pos < pool_pos
            && pool_pos < proc_pos,
        "{context} MUST serialize detail → gate → host_watch → pool → proc-scan (AC-007.87); got: {stdout}"
    );
}

/// FR-007 / AC-007.87 — proc --pid --watch text keeps stderr silent across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_watch_text_stderr_silent() {
    let pid = std::process::id();
    let mut child = bin()
        .args(["proc", "--pid", &pid.to_string(), "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --pid --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, DETAIL_HEADER, 2, Duration::from_millis(30_000));

    assert!(
        stderr.is_empty(),
        "proc --pid --watch text MUST keep stderr silent (AC-007.87); stderr: {stderr:?}"
    );
    let frame_count = stdout.matches(DETAIL_HEADER).count();
    assert!(
        frame_count >= 2,
        "proc --pid --watch MUST re-render at least twice; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "proc --pid --watch text MUST include [watch] footer on stdout (AC-007.87); got: {stdout}"
    );
    assert_text_envelope_order(&stdout, "proc --pid --watch");
    for (idx, segment) in stdout.split(DETAIL_HEADER).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) {
            assert_gate_before_watch(segment, &format!("proc --pid --watch frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.87 — proc --pid --watch --json NDJSON stderr gate → host_watch companions.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_watch_ndjson_stderr_gate_before_host_watch() {
    let pid = std::process::id();
    let mut child = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --pid --json --watch 1");

    let (_stdout, stderr) = drain_watch_pipes(&mut child, DETAIL_HEADER, 2, Duration::from_millis(30_000));

    assert!(
        stderr.contains(GATE_MARKER),
        "proc --pid watch NDJSON stderr MUST include gate companion (AC-007.87); stderr: {stderr}"
    );
    assert!(
        stderr.contains(WATCH_MARKER),
        "proc --pid watch NDJSON stderr MUST include host watch companion (AC-007.87); stderr: {stderr}"
    );
    assert_gate_before_watch(&stderr, "proc --pid watch NDJSON stderr");
    assert!(
        stderr.contains("[watch]"),
        "proc --pid watch NDJSON stderr MUST include [watch] footer; stderr: {stderr}"
    );
}

/// FR-007 / AC-007.87 — proc --pid --watch --json stdout stays pipe-clean NDJSON.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_watch_ndjson_stdout_pipe_clean() {
    let pid = std::process::id();
    let mut child = bin()
        .args(["proc", "--pid", &pid.to_string(), "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --pid --json --watch 1");

    let (stdout, _stderr) = drain_watch_pipes(&mut child, DETAIL_HEADER, 2, Duration::from_millis(30_000));

    assert!(
        !stdout.contains(GATE_MARKER),
        "proc --pid watch NDJSON stdout MUST NOT leak gate companion (AC-007.87); got: {stdout}"
    );
    assert!(
        !stdout.contains(WATCH_MARKER),
        "proc --pid watch NDJSON stdout MUST NOT leak host watch companion (AC-007.87); got: {stdout}"
    );
    assert!(
        !stdout.contains("[watch]"),
        "proc --pid watch NDJSON stdout MUST NOT contain watch footer (AC-007.87); got: {stdout}"
    );
    assert!(
        !stdout.contains("\x1b[2J"),
        "proc --pid watch NDJSON stdout MUST NOT contain terminal clear sequences"
    );

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        !lines.is_empty(),
        "proc --pid watch NDJSON MUST emit at least one line; got: {stdout}"
    );
    for line in lines {
        let v: serde_json::Value =
            serde_json::from_str(line.trim()).expect("each NDJSON stdout line MUST parse");
        assert!(v.get("ts").is_some(), "NDJSON line MUST include ts");
        assert!(v.get("pid").is_some(), "NDJSON line MUST include pid detail");
        assert!(v.get("gate").is_some(), "NDJSON line MUST include gate");
        assert!(v.get("host_watch").is_some(), "NDJSON line MUST include host_watch");
        assert!(v.get("pool").is_some(), "NDJSON line MUST include pool (AC-007.87)");
        assert!(v.get("status").is_some(), "NDJSON line MUST include status (AC-007.87)");
        let gate_pos = line.find("\"gate\"").expect("gate key in raw JSON");
        let host_pos = line.find("\"host_watch\"").expect("host_watch key in raw JSON");
        assert!(
            gate_pos < host_pos,
            "NDJSON line MUST serialize gate before host_watch (AC-007.87); got: {line}"
        );
    }
}
