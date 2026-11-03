// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal gate on `sharecli proc` flat text surfaces
//! FR: FR-007
//!
//! AC-007.21 `proc` text prints gate section before host watch footer
//! (parity with tree AC-007.20 and pid detail AC-007.17)

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-007 / AC-007.21 — proc text prints thermal gate section before host watch.
#[test]
#[serial_test::serial]
fn fr007_proc_text_gate_section() {
    let out = bin().args(["proc"]).output().expect("spawn sharecli proc");
    assert!(out.status.success(), "proc MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("=== Thermal Gate (FR-011) ==="),
        "proc text MUST include gate section (AC-007.21); got: {s}"
    );
    assert!(
        s.contains("Gate decision:"),
        "proc text MUST include gate decision (AC-007.21); got: {s}"
    );
    let gate_pos = s.find("=== Thermal Gate (FR-011) ===").expect("gate section");
    let watch_pos = s.find("=== Host Resource Watch ===").expect("host watch section");
    assert!(
        gate_pos < watch_pos,
        "gate section MUST precede host watch footer (AC-007.21); got: {s}"
    );
}
