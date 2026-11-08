// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — one-shot `sharecli health` / `sharecli pool` text stderr silence
//! FR: FR-007
//!
//! AC-007.37 `health` and `pool` MUST NOT print gate or host_watch text companions on stderr;
//! gate/host_watch stay on stdout only (parity with AC-007.36 status / AC-007.34 proc text).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.37); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.37); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.37); stderr: {s}"
    );
}

fn assert_text_body_has_gate_and_host_watch(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(GATE_MARKER),
        "{context} text body MUST include gate section (AC-007.37); got: {s}"
    );
    assert!(
        s.contains(WATCH_MARKER),
        "{context} text body MUST include host watch section (AC-007.37); got: {s}"
    );
    let gate_pos = s.find(GATE_MARKER).expect("gate section");
    let watch_pos = s.find(WATCH_MARKER).expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "{context} gate section MUST precede host watch footer (AC-007.37); got: {s}"
    );
}

/// FR-007 / AC-007.37 — one-shot health text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_health_text_stderr_silent() {
    let out = bin().args(["health"]).output().expect("spawn sharecli health");
    assert!(out.status.success(), "health MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "health");
    assert_stderr_no_companion_markers(&out.stderr, "health");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "health");
}

/// FR-007 / AC-007.37 — one-shot pool text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_pool_text_stderr_silent() {
    let out = bin().args(["pool"]).output().expect("spawn sharecli pool");
    assert!(out.status.success(), "pool MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "pool");
    assert_stderr_no_companion_markers(&out.stderr, "pool");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "pool");
}
