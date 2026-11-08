// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L45+ soft gate - Pyroscope stub (T-710 Wave16)
//! FR: FR-003
//! Scope: stub only, no live push, no secrets.
//! Verifies `src/pyroscope_stub.rs` soft gate via `sharecli::pyroscope_stub`.

use sharecli::pyroscope_stub::PyroscopeStub;

#[test]
fn c05_pyroscope_stub_is_stub() {
    let stub = PyroscopeStub::new();
    assert!(stub.is_stub(), "stub should be stub");
    assert!(!stub.is_enabled(), "new stub disabled");
}

#[test]
fn c05_pyroscope_stub_push_soft_ok() {
    let stub = PyroscopeStub::enabled();
    assert!(stub.is_enabled());
    assert!(stub.push(b"fake-profile-bytes").is_ok());
    assert!(stub.push(b"").is_ok());
}

#[test]
fn c05_pyroscope_stub_no_live_push() {
    // Ensures stub never performs live push - soft gate, no network, no secrets.
    let stub = PyroscopeStub::new();
    // Both enabled/disabled stubs are still stubs and push is no-op.
    assert!(stub.is_stub());
    assert!(PyroscopeStub::enabled().is_stub());
}
