//! sharecli-fuse — IO-interception tier of the sharecli hypervisor.
//!
//! This crate implements a FUSE filesystem that sits between agent processes and
//! the real backing filesystem.  By intercepting VFS calls at the FUSE layer we
//! can:
//!
//! * Coalesce redundant reads across concurrent agent sessions.
//! * Cache hot paths (Cargo registry, node_modules, build artefacts) in RAM or
//!   on a fast local device, routing cold misses to the backing path.
//! * Remember negative dentries (ENOENT) with TTL so repeated missing-path
//!   lookups skip backing stats until create/mkdir/rename invalidates them.
//! * Meter and throttle per-process IO to prevent one agent's build from starving
//!   another's.
//! * Record provenance — every write carries a (session-id, timestamp) annotation
//!   in the extended-attribute namespace — without modifying the backing FS.
//!
//! The implementation uses [`fuser`] on Linux/macOS (libfuse3 / macFUSE) and
//! [`winfsp`] on Windows (AC-009.25). Mount entry points are gated behind
//! `#[cfg(any(target_os = "linux", target_os = "macos", windows))]`; all other
//! targets compile to a stub that returns an unsupported-platform error.
//!
//! Ported/inspired by the harness-fuse Rust prototype; rewritten on top of
//! `fuser` (vs the earlier Linux-only libfuse ELF binary) to keep the codebase
//! portable and maintainable in pure Rust.

#![warn(missing_docs)]

mod agent_cow;
mod agents_conf;
mod backend;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod cow_session;
mod inode_map;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod mount_smoke;
mod neg_dentry;
mod path_remap;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod provenance;
mod read_cache;
mod session_registry;
#[cfg(windows)]
mod winfsp_mount;
mod write_serialize;
mod write_serialize_meters;

pub use agent_cow::{AgentCowStore, AgentPending};
pub use agents_conf::{sanitize_agent_id, AgentsConf};
pub use backend::{
    probe_runtime, select_backend, select_backend_for_mount, select_backend_for_mount_with,
    select_backend_with, FuseBackend, FuseBackendDiagnostic, FuseBackendSelection,
    FuseCapabilities, FuseRuntimeEvidence,
};
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub use cow_session::CowMountHandle;
pub use inode_map::{abs_under, join_rel, InodeMap, ROOT_INO};
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub use mount_smoke::{
    force_unmount, fuse_mount_smoke_enabled, run_mount_smoke, verify_mount_smoke_provenance,
    MountSession, ENV_FUSE_MOUNT_SMOKE,
};
pub use neg_dentry::{
    global_neg_dentry_meters, NegDentryMeters, NegativeDentryCache, DEFAULT_NEG_TTL,
};
pub use path_remap::remap_mount_to_backing;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub use provenance::{
    annotate_write, annotate_write_at, default_session_id, read_provenance, WriteProvenance,
    ATTR_SESSION, ATTR_WRITTEN_AT,
};
pub use read_cache::{global_read_cache_meters, ReadCacheMeters, ReadContentCache};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use session_registry::default_fuser_config;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use session_registry::smoke_fuser_config;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use session_registry::smoke_fuser_config_for_backend;
pub use session_registry::{FuseMountInfo, FuseMountOptions, FuseSessionRegistry};
#[cfg(windows)]
pub use winfsp_mount::winfsp_installed;
pub use write_serialize::{WriteSerialize, WriteSerializeError};
pub use write_serialize_meters::{
    global_write_serialize_meters, record_commit, record_discard, record_passthrough_write,
    record_stage, WriteSerializeMeters,
};

/// Construction options for [`InterceptFs`] (Feb `harness-fuse` mount flags).
#[derive(Debug, Clone)]
pub struct InterceptFsOptions {
    /// Write-provenance session id.
    pub session_id: String,
    /// Enable per-agent CoW overlays (Feb `--cow`).
    pub cow: bool,
    /// CoW root directory (default: `{backing}/.sharecli-cow` when `cow`, else staging).
    pub cow_dir: Option<std::path::PathBuf>,
    /// Default agent id for unscoped stage/commit (default: session id).
    pub agent: Option<String>,
    /// Per-path write locks (Feb `--no-serialize` clears this).
    pub serialize: bool,
    /// Optional path to Feb-format `agents.conf`.
    pub agents_conf: Option<std::path::PathBuf>,
}

impl Default for InterceptFsOptions {
    fn default() -> Self {
        Self {
            session_id: "default".to_string(),
            cow: false,
            cow_dir: None,
            agent: None,
            serialize: true,
            agents_conf: None,
        }
    }
}

use std::path::Path;

// ---------------------------------------------------------------------------
// Platform-gated implementation
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "linux", target_os = "macos"))]

mod platform;

/// Passthrough FUSE filesystem; attach point for sharecli hypervisor hooks.
///
/// See the crate-level documentation for the full IO-interception design.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use platform::InterceptFs;

/// Mount the sharecli FUSE layer at `mountpoint` over `backing`.
///
/// On Linux and macOS this calls `fuser::mount`; on Windows this uses WinFsp
/// (AC-009.25). Other platforms return an unsupported-platform error.
pub fn mount(mountpoint: &Path, backing: &Path) -> anyhow::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        mount_with_session(mountpoint, backing, &default_session_id())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (mountpoint, backing);
        anyhow::bail!("sharecli-fuse is only supported on Linux, macOS, and Windows (WinFsp)")
    }
}

/// Mount the sharecli FUSE layer with an explicit write-provenance session id.
///
/// Hypervisor cache-miss spawns pass a coalesce-derived session so FUSE writes
/// correlate with the Lock-Wait-Cache key (AC-009.12).
pub fn mount_with_session(
    mountpoint: &Path,
    backing: &Path,
    session_id: &str,
) -> anyhow::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        platform::mount_with_session(mountpoint, backing, session_id)
    }
    #[cfg(windows)]
    {
        let handle = std::sync::Arc::new(crate::CowMountHandle::from_options(
            backing,
            &InterceptFsOptions {
                session_id: session_id.to_string(),
                cow: false,
                cow_dir: None,
                agent: None,
                serialize: true,
                agents_conf: None,
            },
        ));
        winfsp_mount::mount_blocking(mountpoint, backing, session_id, handle)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (mountpoint, backing, session_id);
        anyhow::bail!("sharecli-fuse is only supported on Linux, macOS, and Windows (WinFsp)")
    }
}

#[cfg(test)]

#[cfg(test)]
mod tests;
