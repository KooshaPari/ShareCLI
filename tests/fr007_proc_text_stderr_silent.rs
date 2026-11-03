// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — one-shot `sharecli proc` text stderr silence (inverse of AC-007.28/29)
//! FR: FR-007
//!
//! AC-007.34 `proc`, `proc --tree`, and `proc --pid N` (no `--watch`) MUST NOT print gate or
//! host_watch text companions on stderr; gate/host_watch stay on stdout only (parity with
//! AC-007.30 / AC-007.31 / AC-007.32 / AC-007.33; extends AC-007.17 / AC-007.20 / AC-007.21).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.34); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.34); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.34); stderr: {s}"
    );
}

fn assert_text_body_has_gate_and_host_watch(stdout: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stdout);
    assert!(
        s.contains(GATE_MARKER),
        "{context} text body MUST include gate section (AC-007.34); got: {s}"
    );
    assert!(
        s.contains(WATCH_MARKER),
        "{context} text body MUST include host watch section (AC-007.34); got: {s}"
    );
    let gate_pos = s.find(GATE_MARKER).expect("gate section");
    let watch_pos = s.find(WATCH_MARKER).expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "{context} gate section MUST precede host watch footer (AC-007.34); got: {s}"
    );
}

/// FR-007 / AC-007.34 — one-shot proc text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_proc_text_stderr_silent() {
    let out = bin().args(["proc"]).output().expect("spawn sharecli proc");
    assert!(out.status.success(), "proc MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "proc");
    assert_stderr_no_companion_markers(&out.stderr, "proc");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "proc");
}

/// FR-007 / AC-007.34 — one-shot proc --tree text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_text_stderr_silent() {
    let out = bin().args(["proc", "--tree"]).output().expect("spawn sharecli proc --tree");
    assert!(out.status.success(), "proc --tree MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "proc --tree");
    assert_stderr_no_companion_markers(&out.stderr, "proc --tree");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "proc --tree");
}

/// FR-007 / AC-007.34 — one-shot proc --pid text keeps stderr silent; gate/host_watch on stdout only.
#[test]
#[serial_test::serial]
fn fr007_proc_pid_text_stderr_silent() {
    let pid = std::process::id();
    let out = bin()
        .args(["proc", "--pid", &pid.to_string()])
        .output()
        .expect("spawn sharecli proc --pid");
    assert!(out.status.success(), "proc --pid MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "proc --pid");
    assert_stderr_no_companion_markers(&out.stderr, "proc --pid");
    assert_text_body_has_gate_and_host_watch(&out.stdout, "proc --pid");
}
