// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — one-shot `sharecli status` text stderr silence (inverse of watch NDJSON companions)
//! FR: FR-007
//!
//! AC-007.36 `status` (no `--json`) MUST NOT print gate or host_watch text companions on stderr;
//! gate/host_watch stay on stdout only (parity with AC-007.30 / AC-007.31 / AC-007.32 /
//! AC-007.34; extends AC-007.27 text gate ordering with pipe-clean stderr).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.36); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.36); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.36); stderr: {s}"
    );
}

fn assert_text_body_has_gate_and_host_watch(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(GATE_MARKER),
        "{context} text body MUST include gate section (AC-007.36); got: {s}"
    );
    assert!(
        s.contains(WATCH_MARKER),
        "{context} text body MUST include host watch section (AC-007.36); got: {s}"
    );
    let gate_pos = s.find(GATE_MARKER).expect("gate section");
    let watch_pos = s.find(WATCH_MARKER).expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "{context} gate section MUST precede host watch footer (AC-007.27 / AC-007.36); got: {s}"
    );
}

/// FR-007 / AC-007.36 — one-shot status text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_status_text_stderr_silent() {
    let out = bin().args(["status"]).output().expect("spawn sharecli status");
    assert!(out.status.success(), "status MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "status");
    assert_stderr_no_companion_markers(&out.stderr, "status");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "status");
}
