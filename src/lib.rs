//! sharecli - Shared CLI process manager
//!
//! Thin CLI wrapper around local process runtime.
//!
//! Features:
//! - Process management via local runtime types
//! - Multi-project orchestration

// --- Tier A/B: product + ops (root pub surface) ---
pub mod agent_call_policy;
pub mod audit_log;
pub mod cast;
pub mod commands;
pub mod config;
pub mod config_loader;
pub mod config_merger;
pub mod config_validator;
pub mod config_watcher;
pub mod coordination;
pub mod dashboard_assets;
pub mod env_manager;
pub mod error;
pub mod error_envelope;
pub mod health;
pub mod health_check;
pub mod http_red;
pub mod log_sink;
pub mod metrics;
pub mod monitoring;
pub mod notifier;
pub mod otel;
pub mod pool_index {
    pub use sharecli_sync::PoolIndex;
}
pub mod paths;
pub mod pprof_http;
pub mod proc_table;
pub mod progress;
pub mod pyroscope_stub;
pub mod runtime;
pub mod scheduler;
pub mod serve_auth;
pub mod serve_lock;
pub mod serve_rate_limit;
pub mod session;
mod shutdown;
pub mod signals;
pub mod spawn_policy;
pub mod tray_http;
pub mod watchdog;

/// Parity / expansion utilities. C00 L1 Phase 1 — `docs/ops/lib-sprawl-plan.md`.
pub mod util;

pub use anyhow::Result;
pub use runtime::{
    ManagedProcess, ProcessFilter, ProcessInfo, ProcessPool, ProjectLimits, ProjectResources,
    SharedRuntime,
};
// Compatibility re-exports (Phase 1 non-breaking): legacy `sharecli::<mod>` paths.
pub use util::ansi;
pub use util::api;
pub use util::backoff;
pub use util::binary_search;
pub use util::cache;
pub use util::color;
pub use util::deque;
pub use util::graph;
pub use util::hash_util;
pub use util::lazy;
pub use util::macho_parse;
pub use util::retry;
pub use util::slice_ext;
pub use util::uuid;

#[cfg(test)]
pub mod proptest_util;
pub use util::rate_limiter;
