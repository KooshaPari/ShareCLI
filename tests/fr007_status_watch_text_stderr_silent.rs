//! FR-007 — `sharecli status --watch` text stderr silence (inverse of AC-007.66 JSON)
//! FR: FR-007
//!
//! AC-007.66 `status --watch` (text mode, no `--json`) MUST NOT print gate or host_watch
//! text companions on stderr during refresh cycles; gate/host_watch and `[watch]` footer stay
//! on stdout only (parity with AC-007.65 pool text watch stderr silence).

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";
const STATUS_HEADER: &str = "=== Process Status ===";

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
        "{context} MUST NOT print gate/host_watch companions on stderr during refresh (AC-007.66); stderr: {stderr:?}"
    );
}

fn assert_stderr_no_companion_markers(stderr: &str, context: &str) {
    assert!(
        !stderr.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.66); stderr: {stderr}"
    );
    assert!(
        !stderr.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.66); stderr: {stderr}"
    );
    assert!(
        !stderr.contains("[watch]"),
        "{context} stderr MUST NOT include [watch] footer (AC-007.66); stderr: {stderr}"
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
        "{context} gate section MUST precede host watch footer (AC-007.66); got: {segment}"
    );
}

fn assert_text_watch_stdout(stdout: &str, context: &str) {
    let frame_count = stdout.matches(STATUS_HEADER).count();
    assert!(
        frame_count >= 2,
        "{context} MUST re-render at least twice in dwell window; got {frame_count} frames in: {stdout}"
    );
    assert!(
        stdout.contains(GATE_MARKER),
        "{context} stdout MUST include gate section (AC-007.66); got: {stdout}"
    );
    assert!(
        stdout.contains(WATCH_MARKER),
        "{context} stdout MUST include host watch section (AC-007.66); got: {stdout}"
    );
    assert!(
        stdout.contains("[watch]"),
        "{context} stdout MUST include [watch] footer (AC-007.66); got: {stdout}"
    );
    for (idx, segment) in stdout.split(STATUS_HEADER).skip(1).enumerate() {
        if segment.contains(WATCH_MARKER) {
            assert_gate_before_watch(segment, &format!("{context} frame {}", idx + 1));
        }
    }
}

/// FR-007 / AC-007.66 — status --watch text keeps stderr silent across refresh cycles.
#[test]
#[serial_test::serial]
fn fr007_status_watch_text_stderr_silent() {
    let mut child = bin()
        .args(["status", "--watch", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sharecli status --watch 1");

    let (stdout, stderr) = drain_watch_pipes(&mut child, Duration::from_millis(20_000));

    assert_stderr_silent(&stderr, "status --watch");
    assert_stderr_no_companion_markers(&stderr, "status --watch");
    assert_text_watch_stdout(&stdout, "status --watch");
}
