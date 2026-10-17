//! Capability-gated terminal surface adapters.

use std::sync::Arc;

use anyhow::Result;

use crate::{ledger::SurfaceCapabilities, LayoutRestoreReport, LayoutSnapshot, SurfaceRecord};

/// A discovered terminal surface and its capabilities.
pub trait SurfaceAdapter: Send + Sync {
    /// Report the capabilities this adapter can offer for `surface`.
    fn capabilities(&self, surface: &SurfaceRecord) -> Result<SurfaceCapabilities>;
    /// Enumerate every surface the adapter can observe right now.
    fn discover(&self) -> Result<Vec<SurfaceRecord>>;
    /// Capture the adapter's current pane topology.
    fn snapshot_layout(&self) -> Result<LayoutSnapshot> {
        anyhow::bail!("surface layout snapshot unavailable")
    }
    /// Restore a previously validated pane topology.
    fn restore_layout(&self, snapshot: &LayoutSnapshot) -> Result<LayoutRestoreReport> {
        snapshot.validate()?;
        anyhow::bail!("surface layout restore unavailable")
    }
}

/// Targeted input/output operations for a terminal surface.
pub trait SurfaceIo: Send + Sync {
    /// Send bytes to the surface's input stream.
    fn send(&self, surface_id: &str, bytes: &[u8]) -> Result<()>;
    /// Read a bounded snapshot from the surface's output stream.
    ///
    /// Adapters without readback must return an explicit error; callers must
    /// not interpret an empty buffer as proof that the surface is idle.
    fn read(&self, surface_id: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let _ = (surface_id, max_bytes);
        anyhow::bail!("surface readback unavailable")
    }
    /// Resize the surface's PTY or terminal viewport.
    fn resize(&self, surface_id: &str, rows: u16, cols: u16) -> Result<()>;
}

/// Explicitly degraded Ghostty adapter until a native control transport is proven.
#[derive(Clone, Debug, Default)]
pub struct GhosttySurfaceAdapter {
    /// Whether the adapter has a native RPC channel into Ghostty.
    pub native_rpc: bool,
    /// Whether the adapter can drive Ghostty via Apple Events as a fallback.
    pub apple_events: bool,
}

impl SurfaceAdapter for GhosttySurfaceAdapter {
    fn capabilities(&self, _surface: &SurfaceRecord) -> Result<SurfaceCapabilities> {
        Ok(SurfaceCapabilities {
            read: self.native_rpc,
            write: self.native_rpc || self.apple_events,
            resize: self.native_rpc || self.apple_events,
            layout: self.native_rpc || self.apple_events,
            durable_pty: false,
        })
    }

    fn discover(&self) -> Result<Vec<SurfaceRecord>> {
        if !self.native_rpc && !self.apple_events {
            anyhow::bail!("Ghostty surface discovery unavailable: native transport not configured")
        }
        Ok(Vec::new())
    }
}

/// Managed PTY adapter used for ShareCLI-owned zmx sessions.
#[derive(Clone, Debug)]
pub struct ZmxSurfaceAdapter {
    /// Whether the adapter believes zmx is reachable for IO operations.
    pub available: bool,
    /// Pre-discovered surfaces (used for tests and zmx-less fallbacks).
    pub surfaces: Arc<Vec<SurfaceRecord>>,
}

impl SurfaceAdapter for ZmxSurfaceAdapter {
    fn capabilities(&self, _surface: &SurfaceRecord) -> Result<SurfaceCapabilities> {
        Ok(SurfaceCapabilities {
            read: self.available,
            write: self.available,
            resize: self.available,
            layout: false,
            durable_pty: self.available,
        })
    }

    fn discover(&self) -> Result<Vec<SurfaceRecord>> {
        Ok((*self.surfaces).clone())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn surface() -> SurfaceRecord {
        SurfaceRecord {
            id: "ghostty:1".into(),
            terminal: "ghostty".into(),
            title: None,
            cwd: PathBuf::from("/tmp"),
            process: None,
        }
    }

    #[test]
    fn ghostty_without_native_transport_is_explicitly_degraded() {
        let adapter = GhosttySurfaceAdapter::default();
        assert!(adapter.discover().is_err());
        assert!(!adapter.capabilities(&surface()).unwrap().read);
    }

    #[test]
    fn zmx_advertises_durable_pty_only_when_available() {
        let adapter = ZmxSurfaceAdapter { available: true, surfaces: Arc::new(vec![surface()]) };
        assert!(adapter.capabilities(&surface()).unwrap().durable_pty);
    }
}
