// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-008 / AC-008.11 — Hypervisor coalesce operator meters in `sharecli status`.
//!
//! AC-008.11: `sharecli status` surfaces process-wide coalesce hit/miss/nocache
//! counters via [`global_coalesce_meters`](sharecli_ipc::global_coalesce_meters).

fn sharecli_bin() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

/// FR-008 / AC-008.11 — status prints Hypervisor coalesce section.
#[test]
fn fr008_status_includes_coalesce_section() {
    let out = sharecli_bin().arg("status").output().expect("status");
    assert!(out.status.success(), "status failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("=== Hypervisor Coalesce ==="),
        "status MUST surface coalesce section (AC-008.11); got: {text}"
    );
    assert!(
        text.contains("Cache hits:")
            && text.contains("Cache misses:")
            && text.contains("Nocache runs:")
            && text.contains("Hit rate:"),
        "status MUST include coalesce meter fields (AC-008.11); got: {text}"
    );
    assert!(
        text.contains("=== Hypervisor SlotQueue ===")
            && text.contains("Acquires:")
            && text.contains("Waits:")
            && text.contains("Timeouts:"),
        "status MUST include SlotQueue meter fields (AC-008.12); got: {text}"
    );
}
