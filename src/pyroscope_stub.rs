// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 L45+ soft Pyroscope push stub (Wave16 T-710)
//!
//! Scope: stub only, no live push, no secrets, no network.
//! Provides a no-op interface that can be soft-gated in tests.
//! Real Pyroscope push (live PD) remains `Gap` and requires user infra.

/// Stub client for Pyroscope push. No network, no secrets.
#[derive(Debug, Default, Clone)]
pub struct PyroscopeStub {
    enabled: bool,
}

impl PyroscopeStub {
    /// Create a disabled stub.
    pub fn new() -> Self {
        Self { enabled: false }
    }

    /// Create an enabled stub (still no network).
    pub fn enabled() -> Self {
        Self { enabled: true }
    }

    /// Whether stub is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether this is a stub (always true).
    pub fn is_stub(&self) -> bool {
        true
    }

    /// No-op push. Returns `Ok(())` when stub, never pushes live.
    pub fn push(&self, _payload: &[u8]) -> Result<(), &'static str> {
        if self.is_stub() {
            Ok(())
        } else {
            Err("not a stub")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_is_stub() {
        let s = PyroscopeStub::new();
        assert!(s.is_stub());
    }

    #[test]
    fn stub_push_ok() {
        let s = PyroscopeStub::enabled();
        assert!(s.push(b"profile").is_ok());
    }
}
