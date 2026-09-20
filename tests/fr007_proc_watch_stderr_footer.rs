//! FR-007 — `sharecli proc --watch --json` stderr gate/host_watch companion ordering
//! FR: FR-007
//!
//! AC-007.28 `proc --watch` NDJSON mode prints gate → host_watch companion sections on
//! stderr; stdout stays pipe-clean (no footer or companion leak).

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_gate_before_watch(segment: &str, context: &str) {
    let gate_pos = segment
        .find(GATE_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include gate section; got: {segment}"));
    let watch_pos = segment
        .find(WATCH_MARKER)
        .unwrap_or_else(|| panic!("{context} MUST include host watch section; got: {segment}"));
    assert!(
        gate_pos < watch_pos,
        "{context} MUST print gate before host watch (AC-007.28); got: {segment}"
    );
}

/// FR-007 / AC-007.28 — watch NDJSON stderr carries gate before host_watch companions.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_ndjson_stderr_gate_before_host_watch() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    // Drain stderr on a reader thread, then wait until the first watch tick
    // has emitted the gate + host_watch companions. Fixed sleeps are flaky
    // under parallel CI load (the first tick can exceed the sleep).
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let buf_reader = stderr_buf.clone();
    let mut err_handle = child.stderr.take().expect("stderr handle");
    let reader = thread::spawn(move || {
        let mut s = String::new();
        let _ = err_handle.read_to_string(&mut s);
        *buf_reader.lock().expect("stderr lock") = s;
    });

    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let s = stderr_buf.lock().expect("stderr lock").clone();
        if s.contains(GATE_MARKER) && s.contains(WATCH_MARKER) {
            break;
        }
        if reader.is_finished() || Instant::now() > deadline {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();

    assert!(
        stderr.contains(GATE_MARKER),
        "watch NDJSON stderr MUST include gate companion (AC-007.28); stderr: {stderr}"
    );
    assert!(
        stderr.contains(WATCH_MARKER),
        "watch NDJSON stderr MUST include host watch companion (AC-007.28); stderr: {stderr}"
    );
    assert_gate_before_watch(&stderr, "watch NDJSON stderr");
    assert!(
        stderr.contains("[watch]"),
        "watch NDJSON stderr MUST still include [watch] footer (AC-006.18); stderr: {stderr}"
    );
}

/// FR-007 / AC-007.28 — watch NDJSON stdout stays pipe-clean (no companion or footer leak).
#[test]
#[serial_test::serial]
fn fr007_proc_watch_ndjson_stdout_no_companion_leak() {
    let mut child = bin()
        .args(["proc", "--json", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --json --watch 1");

    // Drain stderr on a reader thread, then wait until the first watch tick
    // has emitted (GATE_MARKER or [watch] footer). The assertion below checks
    // stdout is clean; without this wait the assertion could pass trivially
    // if the watcher hasn't ticked yet — under host load the first tick can
    // exceed a fixed sleep.
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let buf_reader = stderr_buf.clone();
    let mut err_handle = child.stderr.take().expect("stderr handle");
    let reader = thread::spawn(move || {
        let mut s = String::new();
        let _ = err_handle.read_to_string(&mut s);
        *buf_reader.lock().expect("stderr lock") = s;
    });

    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let s = stderr_buf.lock().expect("stderr lock").clone();
        if s.contains(GATE_MARKER) || s.contains("[watch]") {
            break;
        }
        if reader.is_finished() || Instant::now() > deadline {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();

    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    let _ = child.wait();

    assert!(
        !stdout.contains(GATE_MARKER),
        "NDJSON stdout MUST NOT leak gate companion (AC-007.28); got: {stdout}"
    );
    assert!(
        !stdout.contains(WATCH_MARKER),
        "NDJSON stdout MUST NOT leak host watch companion (AC-007.28); got: {stdout}"
    );
    assert!(
        !stdout.contains("[watch]"),
        "NDJSON stdout MUST NOT contain watch footer (AC-007.28); got: {stdout}"
    );
    assert!(!stdout.contains("\x1b[2J"), "NDJSON stdout MUST NOT contain terminal clear sequences");

    for line in stdout.lines().filter(|l| !l.is_empty()) {
        let _: serde_json::Value =
            serde_json::from_str(line).expect("each NDJSON stdout line MUST parse");
    }
}
