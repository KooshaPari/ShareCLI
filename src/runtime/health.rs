//! Health / pool-status value types for `SharedRuntime::health_check` /
//! `SharedRuntime::status`. Kept separate from `shared.rs` so the IPC
//! payload types are discoverable without dragging in the pool impl.

/// Result of `SharedRuntime::health_check` — true if every pooled node/bun
/// process is still alive and within memory limits, plus a list of issues
/// when not.
#[derive(Debug, Clone)]
pub struct RuntimeHealth {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub node_in_use: usize,
    pub bun_in_use: usize,
}

/// Result of `SharedRuntime::status` — counts of pooled processes per
/// harness type and the configured cap.
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub node_total: usize,
    pub node_idle: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub max_per_type: usize,
}
