// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-011 / AC-011.7 — pool + health gate parity with status/report/ps.
//!
//! AC-011.7: `sharecli pool` and `sharecli health` expose thermal+agent gate
//! fields (detected agent count, contention tier, ADMIT/DENY).

fn sharecli_bin() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn assert_gate_section(text: &str, cmd: &str) {
    assert!(
        text.contains("=== Thermal Gate (FR-011) ==="),
        "{cmd} MUST surface gate section (AC-011.7); got: {text}"
    );
    assert!(
        text.contains("Gate decision:"),
        "{cmd} MUST include gate decision (AC-011.7); got: {text}"
    );
    assert!(
        text.contains("Detected agents:"),
        "{cmd} MUST include detected agent count (AC-011.7); got: {text}"
    );
}

/// FR-011 / AC-011.7 — pool prints thermal gate section.
#[test]
fn fr011_pool_includes_gate_section() {
    let out = sharecli_bin().arg("pool").output().expect("pool");
    assert!(out.status.success(), "pool failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert_gate_section(&text, "pool");
}

/// FR-011 / AC-011.7 — health prints thermal gate section.
#[test]
fn fr011_health_includes_gate_section() {
    let out = sharecli_bin().arg("health").output().expect("health");
    assert!(out.status.success(), "health failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert_gate_section(&text, "health");
}
