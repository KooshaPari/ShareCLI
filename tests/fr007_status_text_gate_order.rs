// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate ordering on `sharecli status` text surface
//! FR: FR-007
//!
//! AC-007.27 `status` text prints gate section before host watch
//! (parity with proc text AC-007.21 and status --json AC-007.25)

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const GATE_MARKER: &str = "=== Thermal Gate (FR-011) ===";
const WATCH_MARKER: &str = "=== Host Resource Watch ===";

/// FR-007 / AC-007.27 — status text prints thermal gate section before host watch.
#[test]
#[serial_test::serial]
fn fr007_status_text_gate_before_host_watch() {
    let out = bin().args(["status"]).output().expect("spawn sharecli status");
    assert!(out.status.success(), "status MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains(GATE_MARKER), "status text MUST include gate section (AC-007.27); got: {s}");
    assert!(
        s.contains(WATCH_MARKER),
        "status text MUST include host watch section (AC-007.27); got: {s}"
    );
    let gate_pos = s.find(GATE_MARKER).expect("gate section");
    let watch_pos = s.find(WATCH_MARKER).expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "status text MUST print gate before host watch (AC-007.27); got: {s}"
    );
}
