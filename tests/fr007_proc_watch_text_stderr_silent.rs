//! FR-007 — `sharecli proc --watch` text stderr silence (inverse of AC-007.28/29)
//! FR: FR-007
//!
//! AC-007.35 `proc --watch` and `proc --tree --watch` (no `--json`) MUST NOT print gate or
//! host_watch text companions on stderr during refresh cycles; gate/host_watch and `[watch]`
//! footer stay on stdout only (inverse contract of AC-007.28 / AC-007.29 NDJSON stderr).

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
const INVENTORY_HEADER: &str = "=== Host agents (proc scan) ===";
const TREE_HEADER: &str = "=== Agent process tree (proc scan) ===";

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
    let stdout = stdout_reader.join().expect("stdout drain thread");
    let stderr = stderr_reader.join().expect("stderr drain thread");
    (stdout, stderr)
}

fn assert_stderr_silent(stderr: &str, context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr during refresh (AC-007.35); stderr: {stderr:?}"
    );
}

fn assert_stderr_no_companion_markers(stderr: &str, context: &str) {
    assert!(
        !stderr.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.35); stderr: {stderr}"
    );
    assert!(
        !stderr.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.35); stderr: {stderr}"
    );
    assert!(
        !stderr.contains("[watch]"),
        "{context} stderr MUST NOT include [watch] footer (AC-007.35); stderr: {stderr}"
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
        "{context} gate section MUST precede host watch footer (AC-007.35); got: {segment}"
    );
}

fn assert_text_watch_stdout(stdout: &str, frame_header: &str, context: &str) {
    let frame_count = stdout.matches(frame_header).count();
    assert!(
        frame_count >= 2,
        "{context} MUST re-render at least twice in dwell window; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains(GATE_MARKER),
        "{context} stdout MUST include gate section (AC-007.35); got: {stdout}"
    );
    assert!(
        stdout.contains(WATCH_MARKER),
        "{context} stdout MUST include host watch section (AC-007.35); got: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} stdout MUST include [watch] footer (AC-007.35); got: {stdout}"
    );
    for (idx, segment) in stdout.split(frame_header).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) {
            assert_gate_before_watch(segment, &format!("{context} frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.35 — watch text keeps stderr silent across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_text_stderr_silent() {
    let mut child = bin()
        .args(["proc", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, Duration::from_millis(30_000));

    assert_stderr_silent(&stderr, "proc --watch");
    assert_stderr_no_companion_markers(&stderr, "proc --watch");
    assert_text_watch_stdout(&stdout, INVENTORY_HEADER, "proc --watch");
}

/// FR-007 / AC-007.35 — tree watch text keeps stderr silent across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_watch_text_stderr_silent() {
    let mut child = bin()
        .args(["proc", "--tree", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --tree --watch 1");

    // Wait for the re-render rather than asserting it inside a fixed window.
    //
    // This used drain_watch_pipes(.., 12s) and then asserted ">= 2 frames". That
    // is a throughput assertion: a single `proc --tree` host scan is expensive
    // and, during a parallel full-suite run (215 test binaries plus a busy host),
    // one scan can exceed the entire 12s dwell, producing exactly one frame. The
    // test then failed for being scheduled slowly rather than for misbehaving.
    // With a deadline it still fails if the watch genuinely never re-renders,
    // which is the behaviour AC-007.35 cares about.
    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(60), |buf| {
        buf.matches(TREE_HEADER).count() >= 2
    });

    assert_stderr_silent(&stderr, "proc --tree --watch");
    assert_stderr_no_companion_markers(&stderr, "proc --tree --watch");
    assert_text_watch_stdout(&stdout, TREE_HEADER, "proc --tree --watch");
}

fn drain_watch_until(
    child: &mut Child,
    max_dwell: Duration,
    mut ready: impl FnMut(&str) -> bool,
) -> (String, String) {
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let out_arc = Arc::clone(&stdout_buf);
    let err_arc = Arc::clone(&stderr_buf);
    let stdout_reader = thread::spawn(move || {
        let mut chunk = [0u8; 16_384];
        let mut out = stdout;
        loop {
            match out.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => out_arc
                    .lock()
                    .expect("stdout lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
                Err(_) => break,
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        let mut err = stderr;
        loop {
            match err.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => err_arc
                    .lock()
                    .expect("stderr lock")
                    .push_str(&String::from_utf8_lossy(&chunk[..n])),
                Err(_) => break,
            }
        }
    });
    let deadline = Instant::now() + max_dwell;
    while Instant::now() < deadline {
        let snapshot = stdout_buf.lock().expect("stdout lock").clone();
        if ready(&snapshot) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let stdout = stdout_buf.lock().expect("stdout lock").clone();
    let stderr = stderr_buf.lock().expect("stderr lock").clone();
    (stdout, stderr)
}

/// FR-007 / AC-007.96 — text `[watch]` footer flushes in the same tick as the body.
#[test]
#[serial_test::serial]
fn fr007_proc_watch_text_footer_flushed_same_tick() {
    let mut child = bin()
        .args(["proc", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli proc --watch 1");

    let (stdout, stderr) = drain_watch_until(&mut child, Duration::from_secs(45), |buf| {
        buf.contains(INVENTORY_HEADER) && buf.contains("[watch]")
    });

    assert_stderr_silent(&stderr, "proc --watch text flush");
    let watch_pos =
        stdout.find("[watch]").expect("proc --watch MUST include [watch] footer (AC-007.96)");
    let gates_before = stdout[..watch_pos].matches(GATE_MARKER).count();
    assert_eq!(
        gates_before, 1,
        "AC-007.96: text `[watch]` MUST flush in the same tick (exactly one gate before first footer); got {gates_before} in: {stdout}"
    );
}
