//! Process runtime management with shared pool support.
//!
//! Spawn/monitor/kill delegated to substrate [`ProcessPort`] (`runtime-process`);
//! sharecli retains metadata tagging, pooling, and resource limits.
//!
//! Module structure (post-decomposition):
//!
//! - [`audit`] — `ProcState` enum + `From<ProcessStatus>` + spawn/stop audit
//!   emission (`spawn_capability`, `emit_spawn_audit`, `emit_stop_audit`).
//! - [`process_info`] — `ProcessInfo` + `from_sysinfo` + `count_open_fds` /
//!   `count_threads` + `EnvGuard` (RAII env-var guard).
//! - [`managed`] — `ManagedProcess` (info + handle pair used by `ProcessPool`).
//! - [`pool`] — `ProcessPool` + `ProcessFilter` (the metadata+handle pool).
//! - [`shared`] — `SharedRuntime` + `PooledProcess` (node/bun web-harness pools).
//! - [`health`] — `RuntimeHealth` + `PoolStatus` (IPC payload value types).
//! - [`resources`] — `ProjectLimits` + `ProjectResources` + `ResourceCheck`.
//! - [`tests`] — unit tests (loaded via `#[path]`).

mod audit;
mod health;
mod managed;
mod pool;
mod process_info;
mod resources;
mod shared;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

// Public re-exports — preserve the `crate::runtime::*` surface used by
// `lib.rs` and the various `commands/*` modules.
pub use audit::ProcState;
pub use health::{PoolStatus, RuntimeHealth};
pub use managed::ManagedProcess;
pub use pool::{ProcessFilter, ProcessPool};
pub use process_info::ProcessInfo;
pub use resources::{ProjectLimits, ProjectResources, ResourceCheck};
pub use shared::{PooledProcess, SharedRuntime};

// `pub(crate)` re-exports for items the inline unit tests reference. The
// test module is loaded via `#[path] = "tests.rs"` and lives inside the
// crate, so these are visible to it via `use crate::runtime::*;`.
pub(crate) use audit::spawn_capability;
pub(crate) use process_info::EnvGuard;
