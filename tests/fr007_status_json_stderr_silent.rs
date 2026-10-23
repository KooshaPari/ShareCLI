// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — one-shot `sharecli status --json` stderr silence (inverse of watch NDJSON companions)
//! FR: FR-007
//!
//! AC-007.32 `status --json` MUST NOT print gate or host_watch text companions on stderr;
//! gate/host_watch stay in the JSON body only (parity with AC-007.30 / AC-007.31).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

fn assert_stderr_silent(stderr: &[u8], context: &str) {
    assert!(
        stderr.is_empty(),
        "{context} MUST NOT print gate/host_watch companions on stderr (AC-007.32); stderr: {:?}",
        String::from_utf8_lossy(stderr)
    );
}

fn assert_stderr_no_companion_markers(stderr: &[u8], context: &str) {
    let s = String::from_utf8_lossy(stderr);
    assert!(
        !s.contains(GATE_MARKER),
        "{context} stderr MUST NOT include gate companion text (AC-007.32); stderr: {s}"
    );
    assert!(
        !s.contains(WATCH_MARKER),
        "{context} stderr MUST NOT include host watch companion text (AC-007.32); stderr: {s}"
    );
}

fn assert_json_body_has_gate_and_host_watch(stdout: &[u8], context: &str) {
    let v: serde_json::Value =
        serde_json::from_slice(stdout).expect("{context} MUST emit valid JSON on stdout");
    assert!(v.get("gate").is_some(), "{context} JSON body MUST include gate (AC-007.32); got: {v}");
    assert!(
        v.get("host_watch").is_some(),
        "{context} JSON body MUST include host_watch (AC-007.32); got: {v}"
    );
}

/// FR-007 / AC-007.32 — one-shot status --json keeps stderr silent; gate/host_watch in JSON only.
#[test]
#[serial_test::serial]
fn fr007_status_json_stderr_silent() {
    let out = bin().args(["status", "--json"]).output().expect("spawn sharecli status --json");
    assert!(out.status.success(), "status --json MUST exit 0; stderr: {:?}", out.stderr);
    assert_stderr_silent(&out.stderr, "status --json");
    assert_stderr_no_companion_markers(&out.stderr, "status --json");
    assert_json_body_has_gate_and_host_watch(&out.stdout, "status --json");
}
