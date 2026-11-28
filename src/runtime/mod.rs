// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Process runtime management with shared pool support.
//!
//! Spawn/monitor/kill delegated to substrate [`ProcessPort`] (`runtime-process`);
//! sharecli retains metadata tagging, pooling, and resource limits.
//!
//! Module structure (post-decomposition):
//!
//! - [`admission`] — device-scoped build-slot leasing so `max_concurrent_builds`
//!   holds across sibling `sharecli` invocations, not just within one.
//! - [`audit`] — `ProcState` enum + `From<ProcessStatus>` + spawn/stop audit
//!   emission (`spawn_capability`, `emit_spawn_audit`, `emit_stop_audit`).
//! - [`process_info`] — `ProcessInfo` + `from_sysinfo` + `count_open_fds` /
//!   `count_threads` + `EnvGuard` (RAII env-var guard).
//! - [`managed`] — `ManagedProcess` (info + handle pair used by `ProcessPool`).
//! - [`pool`] — `ProcessPool` + `ProcessFilter` (the metadata+handle pool).
//! - [`registry`] — device-level record of managed pids, so a process started
//!   by one `sharecli` invocation is visible to and killable by the next.
//! - [`shared`] — `SharedRuntime` + `PooledProcess` (node/bun web-harness pools).
//! - [`health`] — `RuntimeHealth` + `PoolStatus` (IPC payload value types).
//! - [`resources`] — `ProjectLimits` + `ProjectResources` + `ResourceCheck`.
//! - [`limits_store`] — persisted `~/.sharecli/limits.json` + measured
//!   `limits check` / `optimize` evaluation.
//! - [`tests`] — unit tests (loaded via `#[path]`).

mod admission;
mod audit;
mod health;
mod limits_store;
mod managed;
mod pool;
mod process_info;
mod registry;
mod resources;
mod shared;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

// Public re-exports — preserve the `crate::runtime::*` surface used by
// `lib.rs` and the various `commands/*` modules.
pub use admission::{SlotLease, SlotPool};
pub use audit::ProcState;
pub use health::{PoolStatus, RuntimeHealth};
pub use limits_store::{
    default_limits_path, detect, evaluate, resolve, suggested_entry, suggestions, DetectionSnapshot,
    DetectionState, FamilyUsage, LimitEntry, LimitRow, LimitSource, LimitVerdict, LimitsFile,
    LimitsStore, ProjectUsage, LIMITS_PATH_ENV,
};
pub use managed::ManagedProcess;
pub use pool::{ProcessFilter, ProcessPool};
pub use process_info::ProcessInfo;
pub use registry::{Registry, RegistryEntry};
pub use resources::{ProjectLimits, ProjectResources, ResourceCheck};
pub use shared::{PooledProcess, SharedRuntime};

// `pub(crate)` re-exports for items the inline unit tests reference. The
// test module is loaded via `#[path] = "tests.rs"` and lives inside the
// crate, so these are visible to it via `use crate::runtime::*;`.
pub(crate) use audit::spawn_capability;
pub(crate) use process_info::EnvGuard;
