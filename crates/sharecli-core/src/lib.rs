// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli-core` — hypervisor engine tier.
//!
//! This crate is the central entry point for spawning managed processes in the
//! sharecli stack.  It wires together the coalescing cache from `sharecli-ipc`
//! (Lock-Wait-Cache deduplication) with real OS process spawning via
//! `tokio::process::Command`.
//!
//! # Architecture
//!
//! ```text
//! caller ──► Hypervisor::run(SpawnRequest)
//!                │
//!                ├─ ThermalGate::poll() ─► Green/Yellow → proceed
//!                │                         Red → sleep-retry (loud) or Err
//!                │
//!                ├─ ResourceWatchSample::capture() ─► FD/net watch on every run
//!                │
//!                ├─ nocache_args match? ─► SlotQueue::with_slot → spawn (no cache)
//!                │
//!                ├─ compute command_key (sharecli-ipc)
//!                │
//!                └─ CoalesceCache::with_lock
//!                       │
//!                       ├─ [cache hit]  → SpawnOutcome { from_cache: true }
//!                       │
//!                       └─ [cache miss] → spawn + store → from_cache: false
//! ```
//!
//! # Thermal gate behaviour
//!
//! Before any spawn the hypervisor queries the [`ThermalGate`](thermal_gate::ThermalGate) trait object:
//!
//! - [`ThermalDecision::Allow`](thermal_gate::ThermalDecision::Allow)  — spawn proceeds normally.
//! - [`ThermalDecision::Warn`](thermal_gate::ThermalDecision::Warn)   — spawn proceeds but a warning is logged.
//! - [`ThermalDecision::Refuse`](thermal_gate::ThermalDecision::Refuse) — spawn is back-pressured.  The hypervisor
//!   enters a visible retry loop ("Waiting for thermal headroom… (N/M)"), and
//!   if the device remains RED after [`THERMAL_MAX_RETRIES`](thermal_gate::THERMAL_MAX_RETRIES) attempts it returns
//!   an explicit `Err`.  This is **never a silent no-op**.
//!
//! # FUSE IO-intercept behaviour
//!
//! Before executing a cache-miss spawn the hypervisor attempts to mount a
//! sharecli-fuse IO-intercept layer over the child's working directory.  When
//! the mount succeeds the child's `cwd` is transparently replaced with the
//! FUSE mountpoint — all filesystem access goes through the intercept layer,
//! which tracks reads/writes for build-system cache sharing.
//!
//! FUSE mounting is **best-effort**: if the platform does not support FUSE
//! (non-Linux/macOS) or the mount fails for any reason, the spawn proceeds
//! without interception.  Cache keys always use the *original* (unwrapped)
//! `cwd` so that identical commands produce the same cache entry regardless
//! of whether FUSE was active.
//!
//! # Module layout
//!
//! - [`hypervisor`] — `Hypervisor` struct, `run` / `run_queued` /
//!   `thermal_gate_check` / `coalesce_via_lock`.
//! - [`types`] — `HypervisorConfig`, `SpawnRequest`, `SpawnOutcome`,
//!   `CachedResult` ↔ `SpawnOutcome` conversions.
//! - [`thermal_gate`] — `ThermalGate` trait, `SystemThermalGate`,
//!   `AgentAwareThermalGate`, `FakeThermalGate`, retry constants.
//! - [`fuse_guard`] — `FuseGuard` RAII mount, `wait_fuse_mount_ready`,
//!   `fuse_session_id_for_command_key`.
//!
//! # TODO hooks (follow-up PRs)
//! - `// TODO(hypervisor): speculative` — pre-execute high-probability commands during
//!   idle periods and pre-populate the coalesce cache.

pub mod detect;
pub mod fuse_guard;
pub mod hypervisor;
pub mod proc_scan;
pub mod speculation;
pub mod thermal_gate;
pub mod transaction_contract;
pub mod types;

#[cfg(test)]
#[path = "tests_hypervisor.rs"]
mod tests_hypervisor;

pub use detect::{match_known_agent, KNOWN_AGENT_FAMILIES};
pub use fuse_guard::fuse_session_id_for_command_key;
pub use hypervisor::Hypervisor;
pub use proc_scan::{
    agent_label_for_pid, detect_caller_agent, is_under_agent, scan_agents, scan_host_agents,
    walk_agent_ancestors, DetectedAgent, FakeProcSource, HostProcSource, ProcSnapshot, ProcSource,
};
pub use sharecli_fleet::{
    sample_host_load_1m, sample_host_net, sample_self_fds, sample_self_rss_bytes,
    ResourceWatchSample,
};
pub use sharecli_ipc::{
    command_key, command_key_with_mode, has_nocache_arg, record_coalesce_lookup_hit,
    record_nocache_run, resolve_operator_queue_priority, semantic_normalize_argv, CacheKeyMode,
    CachedResult, CoalesceCache, CoalesceHitKind, CommandKey, QueuePriority, SlotQueue,
    DEFAULT_NOCACHE_ARGS, QUEUE_PRIORITY_ENV,
};
pub use speculation::{SpeculationTracker, SPECULATION_THRESHOLD, SPECULATION_WINDOW};
pub use thermal_gate::{
    combine_thermal_agent_decision, AgentAwareThermalGate, FakeThermalGate, SystemThermalGate,
    ThermalDecision, ThermalGate, THERMAL_MAX_RETRIES, THERMAL_RETRY_SLEEP,
};
pub use types::{HypervisorConfig, SpawnOutcome, SpawnRequest};
