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
//! Before any spawn the hypervisor queries the [`ThermalGate`] trait object:
//!
//! - [`ThermalDecision::Allow`]  — spawn proceeds normally.
//! - [`ThermalDecision::Warn`]   — spawn proceeds but a warning is logged.
//! - [`ThermalDecision::Refuse`] — spawn is back-pressured.  The hypervisor
//!   enters a visible retry loop ("Waiting for thermal headroom… (N/M)"), and
//!   if the device remains RED after [`THERMAL_MAX_RETRIES`] attempts it returns
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
//! # TODO hooks (follow-up PRs)
//! - `// TODO(hypervisor): speculative` — pre-execute high-probability commands during
//!   idle periods and pre-populate the coalesce cache.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use sharecli_fleet::agent_contention::{
    agent_contention_tier, live_agent_contention_tier, AgentContentionThresholds,
    AgentContentionTier,
};
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use tracing::{debug, error, warn};

pub mod detect;
pub mod proc_scan;
pub mod speculation;
pub use detect::{match_known_agent, KNOWN_AGENT_FAMILIES};
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

// ---------------------------------------------------------------------------
// Thermal gate — trait + decisions
// ---------------------------------------------------------------------------

/// Decision returned by a [`ThermalGate`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalDecision {
    /// Device is cool — spawn may proceed unconditionally.
    Allow,
    /// Device is warm — spawn may proceed but caller should log a warning.
    Warn,
    /// Device is hot — spawn must be back-pressured or refused.
    Refuse,
}

/// A seam that the [`Hypervisor`] calls before every spawn to determine whether
/// the device has enough thermal headroom.
///
/// The production implementation ([`SystemThermalGate`]) delegates to
/// [`ThermalGovernor`] from `sharecli-fleet`.  Tests inject a fake via
/// [`FakeThermalGate`].
pub trait ThermalGate: Send + Sync {
    /// Poll the current thermal state and return a spawn decision.
    fn check(&self) -> ThermalDecision;
}

/// Production [`ThermalGate`] — wraps [`ThermalGovernor`] from `sharecli-fleet`.
///
/// Maps `ThermalLevel::Green` → [`ThermalDecision::Allow`],
///      `ThermalLevel::Yellow` → [`ThermalDecision::Warn`],
///      `ThermalLevel::Red`    → [`ThermalDecision::Refuse`].
///
/// If `poll()` returns an error (e.g. missing sysctl) the gate defaults to
/// [`ThermalDecision::Allow`] and logs a warning so that sysctl unavailability
/// does not block spawns on non-macOS CI runners.
#[derive(Debug)]
pub struct SystemThermalGate {
    governor: ThermalGovernor,
}

impl Default for SystemThermalGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemThermalGate {
    /// Create a new gate backed by the real [`ThermalGovernor`].
    pub fn new() -> Self {
        Self { governor: ThermalGovernor::new() }
    }
}

impl ThermalGate for SystemThermalGate {
    fn check(&self) -> ThermalDecision {
        match self.governor.poll() {
            Ok(ThermalLevel::Green) => ThermalDecision::Allow,
            Ok(ThermalLevel::Yellow) => ThermalDecision::Warn,
            Ok(ThermalLevel::Red) => ThermalDecision::Refuse,
            Err(e) => {
                warn!(err = %e, "thermal-gate: poll failed — defaulting to Allow");
                ThermalDecision::Allow
            }
        }
    }
}

/// Combine hardware thermal gate output with host agent-count contention (FR-011).
pub fn combine_thermal_agent_decision(
    thermal: ThermalDecision,
    agents: AgentContentionTier,
) -> ThermalDecision {
    use AgentContentionTier::{Ok as AgentsOk, Refuse as AgentsRefuse, Warn as AgentsWarn};
    use ThermalDecision::{Allow, Refuse, Warn};
    match (thermal, agents) {
        (Refuse, _) | (_, AgentsRefuse) => Refuse,
        (Allow, AgentsWarn) => Warn,
        (Allow, AgentsOk) => Allow,
        (Warn, _) => Warn,
    }
}

/// Production gate: thermal governor + live host agent inventory escalation.
///
/// Wraps an inner [`ThermalGate`] (typically [`SystemThermalGate`]) and escalates
/// Allow→Warn or any→Refuse when proc-scan agent count or aggregate RSS crosses
/// configured thresholds.
pub struct AgentAwareThermalGate {
    inner: Arc<dyn ThermalGate>,
    agent_tier: Arc<dyn Fn() -> AgentContentionTier + Send + Sync>,
}

impl AgentAwareThermalGate {
    /// Gate with live proc scan + resource samples on every check.
    pub fn new(inner: Arc<dyn ThermalGate>, _thresholds: AgentContentionThresholds) -> Self {
        Self { inner, agent_tier: Arc::new(live_agent_contention_tier) }
    }

    /// Test/injection hook — supply a deterministic agent-count-only tier fn.
    pub fn with_agent_count(
        inner: Arc<dyn ThermalGate>,
        thresholds: AgentContentionThresholds,
        agent_count: fn() -> usize,
    ) -> Self {
        Self {
            inner,
            agent_tier: Arc::new(move || agent_contention_tier(agent_count(), thresholds)),
        }
    }

    /// Test/injection hook — supply an explicit contention tier source.
    pub fn with_agent_tier(
        inner: Arc<dyn ThermalGate>,
        agent_tier: Arc<dyn Fn() -> AgentContentionTier + Send + Sync>,
    ) -> Self {
        Self { inner, agent_tier }
    }
}

impl ThermalGate for AgentAwareThermalGate {
    fn check(&self) -> ThermalDecision {
        let base = self.inner.check();
        let tier = (self.agent_tier)();
        let effective = combine_thermal_agent_decision(base, tier);
        if tier != AgentContentionTier::Ok && effective != base {
            warn!(
                ?tier,
                ?base,
                ?effective,
                "thermal-gate: host agent contention escalated spawn decision"
            );
        }
        effective
    }
}

// ---------------------------------------------------------------------------
// Thermal gate — retry constants
// ---------------------------------------------------------------------------

/// How many times the hypervisor will sleep-retry when the gate returns
/// [`ThermalDecision::Refuse`] before giving up with an explicit error.
pub const THERMAL_MAX_RETRIES: u32 = 5;

/// Duration of each sleep in the thermal back-pressure retry loop.
///
/// 2 s per attempt → up to ~10 s total wait before a hard error is returned.
pub const THERMAL_RETRY_SLEEP: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// FUSE IO-intercept guard
// ---------------------------------------------------------------------------

/// Max time to wait for the FUSE mount to become readable after spawn.
const FUSE_READY_TIMEOUT: Duration = Duration::from_secs(5);
/// Poll interval while waiting for mount readiness.
const FUSE_READY_POLL: Duration = Duration::from_millis(50);
/// Marker filename written under `backing` and probed via the mountpoint.
const FUSE_READY_MARKER: &str = ".sharecli-fuse-ready";

/// Derive the FUSE write-provenance session id for a coalesce [`CommandKey`].
///
/// Hypervisor cache-miss FUSE mounts pass this to [`sharecli_fuse::mount_with_session`]
/// so agent writes through the intercept layer correlate with the Lock-Wait-Cache key
/// (AC-009.12).
pub fn fuse_session_id_for_command_key(key: &CommandKey) -> String {
    let prefix = key.0.get(..16).unwrap_or(key.0.as_str());
    format!("hv-{prefix}")
}

/// Poll until `probe` is readable, `fail_rx` reports a mount error, or `timeout`.
///
/// Returns `Ok(())` when the probe path becomes a readable file. Returns `Err`
/// with a loud, actionable message on mount failure or timeout — never silently
/// treats an unmounted directory as ready.
fn wait_fuse_mount_ready(
    probe: &Path,
    fail_rx: &mpsc::Receiver<String>,
    timeout: Duration,
    poll: Duration,
) -> Result<(), String> {
    let mut waited = Duration::ZERO;
    while waited < timeout {
        if probe.is_file() && std::fs::read(probe).is_ok() {
            return Ok(());
        }
        if let Ok(msg) = fail_rx.try_recv() {
            return Err(format!("sharecli-fuse mount failed: {msg}"));
        }
        thread::sleep(poll);
        waited = waited.saturating_add(poll);
    }
    Err(format!(
        "sharecli-fuse mount readiness timed out after {}ms waiting for {} \
         (is FUSE installed and permitted?)",
        timeout.as_millis(),
        probe.display()
    ))
}

/// RAII guard that manages a sharecli-fuse intercept mount lifetime.
///
/// On construction (best-effort) it creates a temporary directory, spawns a
/// background thread running the FUSE event loop that mirrors a backing path,
/// polls until a readiness marker is visible through the mount, and exposes
/// the mountpoint via [`mountpoint()`][FuseGuard::mountpoint].
///
/// On drop the guard force-unmounts the FUSE filesystem and removes the
/// temporary mountpoint directory.
///
/// # Best-effort semantics
///
/// [`FuseGuard::try_mount`] **never** returns an error to callers.  If mounting
/// fails or readiness polling times out, a loud message is printed to stderr /
/// the error log, a no-op guard is returned, and
/// [`mountpoint()`][FuseGuard::mountpoint] returns `None`. Callers must check
/// `mountpoint()` and fall back to the original path when it returns `None`.
/// An unready mountpoint is **never** returned as `Some`.
struct FuseGuard {
    /// Path to the temporary mountpoint directory.
    /// `None` when FUSE could not be started (no-op guard).
    mountpoint: Option<PathBuf>,
    /// Keep the TempDir alive so it is not cleaned up before the unmount in
    /// [`Drop`] runs.
    _tmpdir: Option<tempfile::TempDir>,
    /// Readiness marker path on the backing tree (removed on drop).
    readiness_marker: Option<PathBuf>,
}

impl FuseGuard {
    /// Attempt to mount a sharecli-fuse IO-intercept layer mirroring `backing`.
    ///
    /// When FUSE is unavailable, the mount fails, or readiness never appears, a
    /// no-op guard is returned (the spawn proceeds without interception) after a
    /// loud failure report.
    fn try_mount(backing: &Path, session_id: &str) -> Self {
        // Non-Linux/macOS: no-op guard — FUSE is not available.
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (backing, session_id);
            return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
        }

        // Linux / macOS: try to start FUSE and wait until it is ready.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let tmpdir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => {
                    let msg = format!(
                        "sharecli-fuse: cannot create mount tempdir: {e} — proceeding without FUSE"
                    );
                    eprintln!("{msg}");
                    error!("{msg}");
                    return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
                }
            };
            let mountpoint = tmpdir.path().to_path_buf();
            let backing = backing.to_path_buf();

            // Unique marker under backing; InterceptFs mirrors it through the mount.
            let marker_name = format!("{FUSE_READY_MARKER}-{}", std::process::id());
            let marker_rel = Path::new(&marker_name);
            let marker_backing = backing.join(marker_rel);
            if let Err(e) = std::fs::write(&marker_backing, b"ready") {
                let msg = format!(
                    "sharecli-fuse: cannot write readiness marker {}: {e} — proceeding without FUSE",
                    marker_backing.display()
                );
                eprintln!("{msg}");
                error!("{msg}");
                return Self { mountpoint: None, _tmpdir: None, readiness_marker: None };
            }

            let mp = mountpoint.clone();
            let backing_for_mount = backing.clone();
            let session = session_id.to_string();
            let (fail_tx, fail_rx) = mpsc::channel::<String>();
            // Spawn the FUSE event loop on a background thread — it blocks
            // until unmounted.
            thread::spawn(move || {
                if let Err(err) =
                    sharecli_fuse::mount_with_session(&mp, &backing_for_mount, &session)
                {
                    let _ = fail_tx.send(err.to_string());
                }
            });

            let probe = mountpoint.join(marker_rel);
            match wait_fuse_mount_ready(&probe, &fail_rx, FUSE_READY_TIMEOUT, FUSE_READY_POLL) {
                Ok(()) => Self {
                    mountpoint: Some(mountpoint),
                    _tmpdir: Some(tmpdir),
                    readiness_marker: Some(marker_backing),
                },
                Err(msg) => {
                    let loud = format!("{msg} — proceeding without FUSE intercept");
                    eprintln!("{loud}");
                    error!("{loud}");
                    // Best-effort unmount of a half-started mount before drop.
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("fusermount")
                            .arg("-uz")
                            .arg(&mountpoint)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("umount")
                            .arg(&mountpoint)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status();
                    }
                    let _ = std::fs::remove_file(&marker_backing);
                    Self { mountpoint: None, _tmpdir: None, readiness_marker: None }
                }
            }
        }
    }

    /// The mountpoint directory, or `None` if the guard is a no-op.
    fn mountpoint(&self) -> Option<&Path> {
        self.mountpoint.as_deref()
    }
}

impl Drop for FuseGuard {
    fn drop(&mut self) {
        if let Some(ref mp) = self.mountpoint {
            // Force-unmount (the `_tmpdir` field is dropped after this,
            // which removes the now-empty mountpoint directory).
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("fusermount").arg("-uz").arg(mp).status();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("umount").arg(mp).status();
            }
        }
        if let Some(ref marker) = self.readiness_marker {
            let _ = std::fs::remove_file(marker);
        }
    }
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for the [`Hypervisor`].
#[derive(Debug, Clone)]
pub struct HypervisorConfig {
    /// Root directory for the coalesce cache.
    pub cache_root: PathBuf,
    /// Root directory for the mutating/nocache [`SlotQueue`].
    pub queue_root: PathBuf,
    /// Max parallel slots for queued (mutating) commands.
    pub queue_max_concurrent: usize,
    /// TTL for coalesce cache entries (origin harness rules.conf `ttl=`).
    pub coalesce_ttl: Duration,
    /// Debounce window for the coalesce miss path (origin harness `debounce_ms`).
    ///
    /// When non-zero, [`CoalesceCache::with_lock`] waits then re-checks so an
    /// in-window sibling store is shared (AC-008.6). [`Duration::ZERO`] disables.
    pub coalesce_debounce: Duration,
    /// Cache-key mode from rules.conf `cache_key=` (AC-008.19).
    pub cache_key_mode: CacheKeyMode,
    /// When true, apply lint-tool semantic argv normalization before hashing (AC-008.20).
    pub semantic: bool,
}

/// A request to spawn a managed process.
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    /// Argument vector — `argv[0]` is the program name.
    pub argv: Vec<String>,
    /// Working directory for the child process.
    pub cwd: PathBuf,
    /// Environment variable overrides passed to the child.
    pub env: Vec<(String, String)>,
    /// Nocache / mutating-path [`SlotQueue`] priority (FR-008 AC-008.14).
    pub queue_priority: QueuePriority,
}

impl SpawnRequest {
    /// Build a spawn request with operator-resolved nocache queue priority.
    pub fn new(argv: Vec<String>, cwd: PathBuf, env: Vec<(String, String)>) -> Self {
        Self::from_operator(argv, cwd, env, None)
    }

    /// Build from argv/cwd/env with optional rules.conf `priority=` fallback.
    ///
    /// Non-empty [`QUEUE_PRIORITY_ENV`] wins over `rule_priority` (FR-008 AC-008.15).
    pub fn from_operator(
        argv: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        rule_priority: Option<&str>,
    ) -> Self {
        Self { argv, cwd, env, queue_priority: resolve_operator_queue_priority(rule_priority) }
    }

    /// Override nocache queue priority for this spawn.
    #[must_use]
    pub fn with_queue_priority(mut self, priority: QueuePriority) -> Self {
        self.queue_priority = priority;
        self
    }
}

/// The outcome of a [`Hypervisor::run`] call.
#[derive(Debug, Clone)]
pub struct SpawnOutcome {
    /// Exit status code of the process (or the cached result).
    pub exit_code: i32,
    /// Raw bytes captured from standard output.
    pub stdout: Vec<u8>,
    /// Raw bytes captured from standard error.
    pub stderr: Vec<u8>,
    /// `true` when the result was served from the coalesce cache without
    /// actually spawning a new process.
    pub from_cache: bool,
    /// Live FD/net watch sample captured at [`Hypervisor::run`] entry (FR-007).
    pub resource_watch: ResourceWatchSample,
    /// Nearest known-agent ancestor for the hypervisor process at spawn time (FR-006).
    pub detected_agent: Option<DetectedAgent>,
    /// FUSE write-provenance session id when intercept was active for this run (AC-009.13).
    ///
    /// `None` for cache hits, nocache queue routing, or cache-miss spawns where FUSE
    /// mount did not become ready.
    pub fuse_session_id: Option<String>,
    /// Original backing cwd before FUSE intercept remap (AC-009.14).
    ///
    /// `Some` only when [`fuse_intercept_active`] is true; pairs with
    /// [`fuse_mountpoint`](Self::fuse_mountpoint) for [`remap_fuse_path`](Self::remap_fuse_path).
    pub fuse_backing: Option<PathBuf>,
    /// Ephemeral FUSE mountpoint used as the child cwd (AC-009.14).
    pub fuse_mountpoint: Option<PathBuf>,
}

impl SpawnOutcome {
    fn with_resource_watch(mut self, watch: ResourceWatchSample) -> Self {
        self.resource_watch = watch;
        self
    }

    fn with_detected_agent(mut self, agent: Option<DetectedAgent>) -> Self {
        self.detected_agent = agent;
        self
    }

    /// Nearest known-agent family from proc-scan ancestor walk, if any (FR-006).
    pub fn agent_family(&self) -> Option<&'static str> {
        self.detected_agent.as_ref().map(|a| a.family)
    }

    /// `true` when a FUSE intercept session was active for this spawn outcome (AC-009.13).
    pub fn fuse_intercept_active(&self) -> bool {
        self.fuse_session_id.is_some()
    }

    /// Remap an absolute or mount-relative path to its backing equivalent (AC-009.14).
    ///
    /// Returns `None` when FUSE intercept was inactive or `path` lies outside the
    /// mount subtree.
    pub fn remap_fuse_path(&self, path: &Path) -> Option<PathBuf> {
        match (&self.fuse_mountpoint, &self.fuse_backing) {
            (Some(mp), Some(bk)) => sharecli_fuse::remap_mount_to_backing(mp, bk, path),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CachedResult ↔ SpawnOutcome conversions (required by CoalesceCache::with_lock)
// ---------------------------------------------------------------------------

impl From<CachedResult> for SpawnOutcome {
    fn from(c: CachedResult) -> Self {
        Self {
            exit_code: c.exit_code,
            stdout: c.stdout,
            stderr: c.stderr,
            from_cache: true,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        }
    }
}

impl From<SpawnOutcome> for CachedResult {
    fn from(s: SpawnOutcome) -> Self {
        Self { exit_code: s.exit_code, stdout: s.stdout, stderr: s.stderr }
    }
}

// ---------------------------------------------------------------------------
// Hypervisor
// ---------------------------------------------------------------------------

/// The sharecli hypervisor engine.
///
/// Owns a [`CoalesceCache`] and routes every [`SpawnRequest`] through the
/// Lock-Wait-Cache protocol: identical concurrent commands coalesce into a
/// single execution, with all waiters receiving the same cached result.
///
/// A [`ThermalGate`] is consulted before every spawn.  When the device is in a
/// RED thermal state the hypervisor enters a visible sleep-retry loop and, if
/// the state does not clear within [`THERMAL_MAX_RETRIES`] attempts, returns an
/// explicit error rather than silently dropping or degrading the spawn.
pub struct Hypervisor {
    cache: CoalesceCache,
    queue: SlotQueue,
    /// Mutating flags that force queue routing (Feb `nocache_args`).
    nocache_args: Vec<String>,
    #[allow(dead_code)]
    config: HypervisorConfig,
    thermal_gate: Arc<dyn ThermalGate>,
    /// FR-008 speculative execution tracker.
    pub speculation_tracker: Arc<SpeculationTracker>,
}

impl Hypervisor {
    /// Create a new `Hypervisor` with its coalesce cache rooted at `cache_root`
    /// and the production thermal gate (hardware + host agent contention).
    ///
    /// Queue root defaults to `{cache_root}/queue` with `max_concurrent = 1`.
    pub fn new(cache_root: impl Into<PathBuf>) -> Self {
        let inner = Arc::new(SystemThermalGate::new());
        let gate =
            Arc::new(AgentAwareThermalGate::new(inner, AgentContentionThresholds::default()));
        Self::with_thermal_gate(cache_root, gate)
    }

    /// Create a `Hypervisor` with an explicit [`ThermalGate`] implementation.
    ///
    /// Intended for tests that inject a [`FakeThermalGate`] (or any other
    /// implementation) to exercise gate behaviour without real hardware.
    pub fn with_thermal_gate(cache_root: impl Into<PathBuf>, gate: Arc<dyn ThermalGate>) -> Self {
        let cache_root = cache_root.into();
        let queue_root = cache_root.join("queue");
        Self::with_options(
            HypervisorConfig {
                cache_root,
                queue_root,
                queue_max_concurrent: 1,
                coalesce_ttl: CoalesceCache::DEFAULT_TTL,
                coalesce_debounce: Duration::ZERO,
                cache_key_mode: CacheKeyMode::Time,
                semantic: false,
            },
            gate,
            DEFAULT_NOCACHE_ARGS.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    /// Production hypervisor from a full [`HypervisorConfig`] (harness rules.conf wiring).
    pub fn from_config(config: HypervisorConfig) -> Self {
        let inner = Arc::new(SystemThermalGate::new());
        let gate =
            Arc::new(AgentAwareThermalGate::new(inner, AgentContentionThresholds::default()));
        Self::with_options(
            config,
            gate,
            DEFAULT_NOCACHE_ARGS.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    /// Test / injection constructor with explicit config and thermal gate.
    pub fn from_config_with_gate(config: HypervisorConfig, gate: Arc<dyn ThermalGate>) -> Self {
        Self::with_options(config, gate, vec![])
    }

    /// Full constructor: coalesce cache + slot queue + nocache flag list + thermal gate.
    ///
    /// Hypervisor / external callers that need a custom queue root or mutating-flag
    /// set should use this (or [`Self::queue`] after construction).
    pub fn with_options(
        config: HypervisorConfig,
        gate: Arc<dyn ThermalGate>,
        nocache_args: Vec<String>,
    ) -> Self {
        let cache = CoalesceCache::with_options(
            config.cache_root.clone(),
            config.coalesce_ttl,
            config.coalesce_debounce,
        );
        let queue = SlotQueue::new(config.queue_root.clone(), config.queue_max_concurrent);

        // FR-008: create speculation tracker and spawn the background task.
        let speculation_tracker = Arc::new(SpeculationTracker::new());
        speculation::spawn_speculation_task(
            Arc::clone(&speculation_tracker),
            cache.clone(),
            gate.clone(),
        );

        Self { cache, queue, nocache_args, config, thermal_gate: gate, speculation_tracker }
    }

    /// Borrow the mutating-path [`SlotQueue`] (Hypervisor API for external callers).
    pub fn queue(&self) -> &SlotQueue {
        &self.queue
    }

    /// Configured coalesce TTL. Wired into every [`Hypervisor::run`] coalesce path.
    pub fn coalesce_ttl(&self) -> Duration {
        self.cache.ttl()
    }

    /// Configured coalesce debounce window (zero = disabled). Wired into every
    /// [`Hypervisor::run`] coalesce path via [`CoalesceCache::with_lock`].
    pub fn coalesce_debounce(&self) -> Duration {
        self.cache.debounce()
    }

    /// Configured SlotQueue parallelism (harness `max_concurrent=`).
    pub fn queue_max_concurrent(&self) -> usize {
        self.config.queue_max_concurrent
    }

    /// Configured nocache / mutating flags (Feb `nocache_args`).
    pub fn nocache_args(&self) -> &[String] {
        &self.nocache_args
    }

    /// Replace the nocache flag list (e.g. from a rules.conf fragment).
    pub fn set_nocache_args(&mut self, flags: Vec<String>) {
        self.nocache_args = flags;
    }

    /// Run exclusively through the SlotQueue lane (harness `queue` / `priority_queue`).
    ///
    /// Skips coalesce cache lookup regardless of argv. Used by harness-native
    /// strategies that must serialize on the nocache lane (FR-008 AC-008.16).
    pub async fn run_queued(&self, req: SpawnRequest, lane: &str) -> Result<SpawnOutcome> {
        self.thermal_gate_check().await?;

        let watch = ResourceWatchSample::capture()?;
        let detected_agent = detect_caller_agent();
        debug!(lane, argv = ?req.argv, "hypervisor::run_queued — queue lane");
        record_nocache_run();
        let outcome =
            self.queue.with_slot(lane, req.queue_priority, || spawn_process_sync(&req))?;
        Ok(SpawnOutcome {
            exit_code: outcome.exit_code,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        }
        .with_resource_watch(watch)
        .with_detected_agent(detected_agent))
    }

    /// Run a managed spawn with Lock-Wait-Cache coalescing.
    ///
    /// # Thermal gating
    /// Before touching the coalesce cache the hypervisor polls [`ThermalGate`]:
    ///
    /// - **Green** → proceed normally.
    /// - **Yellow** → log a warning, then proceed.
    /// - **Red** → print "Waiting for thermal headroom… (N/M)" to stderr and
    ///   sleep [`THERMAL_RETRY_SLEEP`] between each attempt, up to
    ///   [`THERMAL_MAX_RETRIES`] times.  If the gate is still RED after all
    ///   retries, return `Err("spawn refused: device is thermally throttled …")`.
    ///   This is **never a silent no-op**.
    ///
    /// # FUSE IO-intercept
    /// On a cache miss the hypervisor attempts to mount a sharecli-fuse intercept
    /// layer over the child's `cwd`, using a write-provenance session id derived
    /// from the coalesce [`CommandKey`] (AC-009.12).  When the mount succeeds the
    /// child runs against the FUSE mountpoint — all filesystem access goes through
    /// the intercept layer for build-system cache sharing.  FUSE is **best-effort**:
    /// if mounting fails the spawn proceeds without interception.
    ///
    /// # Coalescing behaviour
    /// - If no cached result exists for this command the process is spawned,
    ///   its output captured, and the result stored.
    /// - If a cached result already exists (i.e. an identical command was
    ///   recently run) the result is returned immediately without a new spawn.
    /// - Concurrent callers with the same command key block on an advisory
    ///   flock; the first one to acquire the lock spawns; the rest read the
    ///   cache once the lock is released.
    ///
    /// # FR-008: speculative execution
    /// Command-frequency histograms are tracked in [`SpeculationTracker`];
    /// the background task pre-executes high-probability commands into the
    /// coalesce cache during idle periods.
    pub async fn run(&self, req: SpawnRequest) -> Result<SpawnOutcome> {
        // ── Thermal gate ─────────────────────────────────────────────────────
        self.thermal_gate_check().await?;

        // ── Resource watch (FR-007 live hypervisor path) ───────────────────────
        let watch = ResourceWatchSample::capture()?;
        let detected_agent = detect_caller_agent();
        if let Some(ref agent) = detected_agent {
            debug!(
                pid = agent.pid,
                family = agent.family,
                "hypervisor::run — caller under known agent"
            );
        }
        debug!(
            fd_count = watch.fd_count,
            net_rx_bytes = watch.net_rx_bytes,
            net_tx_bytes = watch.net_tx_bytes,
            mem_rss_bytes = watch.mem_rss_bytes,
            load_1m = watch.load_1m,
            "hypervisor::run — resource watch sample"
        );

        // ── nocache_args → queue (never coalesce) ────────────────────────────
        // Feb harness: if argv contains a mutating flag, fall back to queue.
        if has_nocache_arg(&req.argv, &self.nocache_args) {
            let lane = queue_lane_from_argv(&req.argv).to_string();
            debug!(lane = %lane, argv = ?req.argv, "hypervisor::run — nocache → queue");
            return self.run_queued(req, &lane).await;
        }

        // ── Cache lookup ─────────────────────────────────────────────────────
        // NOTE: the cache key uses the *original* `req.cwd` so that identical
        // commands produce the same key regardless of whether FUSE is active.
        let argv_for_key = if self.config.semantic {
            semantic_normalize_argv(&req.argv, &req.cwd)
        } else {
            req.argv.clone()
        };
        let key =
            command_key_with_mode(self.config.cache_key_mode, &argv_for_key, &req.cwd, &req.env);
        debug!(key = %key.0, argv = ?req.argv, "hypervisor::run");

        // Check the cache before acquiring the lock so that we can
        // accurately report `from_cache` for the caller.
        if let Some(cached) = self.cache.lookup(&key)? {
            debug!(key = %key.0, "hypervisor::run — cache hit");
            record_coalesce_lookup_hit();
            // FR-008: record hit for speculation tracker.
            self.speculation_tracker.record_hit(&key, &req.argv, &req.cwd, &req.env).await;
            return Ok(SpawnOutcome {
                exit_code: cached.exit_code,
                stdout: cached.stdout,
                stderr: cached.stderr,
                from_cache: true,
                resource_watch: ResourceWatchSample::default(),
                detected_agent: None,
                fuse_session_id: None,
                fuse_backing: None,
                fuse_mountpoint: None,
            }
            .with_resource_watch(watch)
            .with_detected_agent(detected_agent.clone()));
        }

        // ── FUSE intercept (cache-miss only) ─────────────────────────────────
        // Mount the IO-intercept layer over the child's working directory.
        // `FuseGuard::try_mount` never fails the spawn — if FUSE is unavailable
        // or readiness never appears, a loud error is reported and a no-op
        // guard is returned so the spawn proceeds without interception.
        let fuse_session = fuse_session_id_for_command_key(&key);
        let fuse_guard = FuseGuard::try_mount(&req.cwd, &fuse_session);
        let fuse_session_id = fuse_guard.mountpoint().map(|_| fuse_session.clone());
        let fuse_backing = fuse_guard.mountpoint().map(|_| req.cwd.clone());
        let fuse_mountpoint = fuse_guard.mountpoint().map(|p| p.to_path_buf());

        // Build an effective SpawnRequest whose cwd points at the FUSE
        // mountpoint (or the original cwd when FUSE is inactive).
        // This is a *separate owned clone* — no borrow relationship to `req`,
        // which avoids the borrow-checker conflict that would arise if we tried
        // to modify `req.cwd` inside the `with_lock` closure below.
        let effective_req = fuse_guard
            .mountpoint()
            .map(|mp| SpawnRequest { cwd: mp.to_path_buf(), ..req.clone() })
            .unwrap_or_else(|| req.clone());

        // Cache miss — acquire the advisory flock, re-check inside the lock
        // (a sibling may have stored the result while we were waiting), and
        // only spawn if still a miss.
        //
        // Cache keys always use the *original* `cwd` so coalescing is stable
        // regardless of whether the FUSE intercept is active.

        // Lock-Wait-Cache: spawn is the closure called only on a cache miss.
        // We use `effective_req` (with a potentially FUSE-wrapped cwd)
        // inside the closure to avoid any borrow conflict with `req`.
        let (cached, hit_kind) = self.coalesce_via_lock(&key, &effective_req)?;

        // FR-008: record speculation hit when the lock-wait cache was shared.
        if hit_kind.shared_from_cache() {
            self.speculation_tracker.record_hit(&key, &req.argv, &req.cwd, &req.env).await;
        }

        Ok(SpawnOutcome {
            exit_code: cached.exit_code,
            stdout: cached.stdout,
            stderr: cached.stderr,
            from_cache: hit_kind.shared_from_cache(),
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id,
            fuse_backing,
            fuse_mountpoint,
        }
        .with_resource_watch(watch)
        .with_detected_agent(detected_agent))
    }

    /// Coalesce miss path: advisory flock + debounce re-check + optional spawn.
    ///
    /// Every Hypervisor coalesce miss MUST flow through here so
    /// [`CoalesceCache::with_lock_detailed`] applies the configured debounce window.
    fn coalesce_via_lock(
        &self,
        key: &sharecli_ipc::CommandKey,
        effective_req: &SpawnRequest,
    ) -> Result<(CachedResult, CoalesceHitKind)> {
        self.cache.with_lock_detailed(key, || {
            let outcome = spawn_process_sync(effective_req)?;
            Ok(CachedResult {
                exit_code: outcome.exit_code,
                stdout: outcome.stdout,
                stderr: outcome.stderr,
            })
        })
    }

    /// Poll the thermal gate with a visible sleep-retry loop on RED.
    ///
    /// Returns `Ok(())` when the gate allows spawning.
    /// Returns `Err` if the gate refuses after all retries.
    async fn thermal_gate_check(&self) -> Result<()> {
        let mut attempt = 0u32;

        loop {
            match self.thermal_gate.check() {
                ThermalDecision::Allow => {
                    debug!("thermal-gate: Green — spawn allowed");
                    return Ok(());
                }
                ThermalDecision::Warn => {
                    warn!("thermal-gate: Yellow — device is warm, proceeding with spawn");
                    return Ok(());
                }
                ThermalDecision::Refuse => {
                    attempt += 1;
                    // Loud, actionable message — never a silent no-op.
                    eprintln!(
                        "sharecli: Waiting for thermal headroom\u{2026} ({attempt}/{THERMAL_MAX_RETRIES})"
                    );
                    warn!(
                        attempt,
                        max = THERMAL_MAX_RETRIES,
                        "thermal-gate: Red — spawn back-pressured"
                    );

                    if attempt >= THERMAL_MAX_RETRIES {
                        return Err(anyhow!(
                            "spawn refused: device is thermally throttled after \
                             {THERMAL_MAX_RETRIES} retries ({sleep}s each). \
                             Reduce concurrent builds or wait for the device to cool down.",
                            sleep = THERMAL_RETRY_SLEEP.as_secs(),
                        ));
                    }

                    tokio::time::sleep(THERMAL_RETRY_SLEEP).await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Process execution
// ---------------------------------------------------------------------------

/// Lane key for SlotQueue routing from argv (basename of argv[0]).
fn queue_lane_from_argv(argv: &[String]) -> &str {
    argv.first().map(|s| s.as_str()).unwrap_or("unknown").rsplit('/').next().unwrap_or("unknown")
}

/// Spawn `req.argv` synchronously (blocking) and capture its output.
///
/// Used inside `CoalesceCache::with_lock` which takes a synchronous closure.
fn spawn_process_sync(req: &SpawnRequest) -> Result<SpawnOutcome> {
    let (program, args) =
        req.argv.split_first().with_context(|| "spawn_process_sync: argv is empty")?;

    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(&req.cwd)
        .envs(req.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .with_context(|| format!("failed to spawn {:?}", req.argv))?;

    let exit_code = output.status.code().unwrap_or(-1);
    Ok(SpawnOutcome {
        exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        from_cache: false,
        resource_watch: ResourceWatchSample::default(),
        detected_agent: None,
        fuse_session_id: None,
        fuse_backing: None,
        fuse_mountpoint: None,
    })
}

// ---------------------------------------------------------------------------
// Test helpers (pub(crate) for tests module; also exported for integration tests)
// ---------------------------------------------------------------------------

/// A controllable [`ThermalGate`] for unit tests.
///
/// The gate's decision is set at construction time and never changes, making it
/// suitable for table-driven tests that need deterministic thermal states.
#[derive(Debug)]
pub struct FakeThermalGate {
    decision: ThermalDecision,
}

impl FakeThermalGate {
    /// Create a fake gate that always returns `decision`.
    pub fn new(decision: ThermalDecision) -> Self {
        Self { decision }
    }
}

impl ThermalGate for FakeThermalGate {
    fn check(&self) -> ThermalDecision {
        self.decision
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use tempfile::TempDir;

    use super::*;

    fn echo_argv(msg: &str) -> Vec<String> {
        // Use a portable shell-free echo: `echo` is available on both unix and Windows.
        #[cfg(unix)]
        return vec!["echo".to_string(), msg.to_string()];
        #[cfg(windows)]
        return vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), msg.to_string()];
    }

    /// Hypervisor with Allow gate and no agent-contention wrapper (unit tests).
    fn allow_hypervisor(dir: &Path) -> Hypervisor {
        Hypervisor::with_thermal_gate(dir, Arc::new(FakeThermalGate::new(ThermalDecision::Allow)))
    }

    // ── Existing cache-coalescing tests ──────────────────────────────────────

    /// (a) Running a simple echo command for the first time should succeed with
    ///     `from_cache = false` and the expected stdout.
    #[tokio::test]
    async fn run_echo_fresh() {
        let dir = TempDir::new().expect("tempdir");
        let hv = allow_hypervisor(dir.path());

        let req = SpawnRequest {
            argv: echo_argv("hello"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        let outcome = hv.run(req).await.expect("run should succeed");

        assert_eq!(outcome.exit_code, 0, "echo should exit 0");
        assert!(!outcome.from_cache, "first run must not come from cache");

        let stdout = String::from_utf8_lossy(&outcome.stdout);
        assert!(stdout.contains("hello"), "stdout should contain 'hello', got: {stdout:?}");
    }

    /// (b) A second identical run must return the cached result (`from_cache = true`)
    ///     with the same stdout bytes — without re-executing the process.
    #[tokio::test]
    async fn run_echo_coalesces_on_second_call() {
        let dir = TempDir::new().expect("tempdir");
        let hv = allow_hypervisor(dir.path());

        let req = SpawnRequest {
            argv: echo_argv("world"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        // First call — live spawn.
        let first = hv.run(req.clone()).await.expect("first run");
        assert!(!first.from_cache, "first run must not come from cache");
        assert_eq!(first.exit_code, 0);

        // Second call — must hit the cache.
        let second = hv.run(req).await.expect("second run");
        assert!(second.from_cache, "second run must come from cache");
        assert_eq!(second.stdout, first.stdout, "cached stdout must match original");
        assert_eq!(second.exit_code, first.exit_code);
    }

    // ── Thermal gate unit tests ──────────────────────────────────────────────

    /// Green gate: spawn must succeed immediately without any retry.
    #[tokio::test]
    async fn thermal_gate_green_allows_spawn() {
        let dir = TempDir::new().expect("tempdir");
        let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
        let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

        let req = SpawnRequest {
            argv: echo_argv("green-gate"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        let outcome = hv.run(req).await.expect("Green gate must allow spawn");
        assert_eq!(outcome.exit_code, 0);
        let stdout = String::from_utf8_lossy(&outcome.stdout);
        assert!(stdout.contains("green-gate"));
    }

    /// Yellow gate: spawn must succeed (warm device does not block).
    #[tokio::test]
    async fn thermal_gate_yellow_allows_spawn_with_warning() {
        let dir = TempDir::new().expect("tempdir");
        let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Warn));
        let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

        let req = SpawnRequest {
            argv: echo_argv("yellow-gate"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        // Yellow must not block or error.
        let outcome = hv.run(req).await.expect("Yellow gate must allow spawn");
        assert_eq!(outcome.exit_code, 0);
        let stdout = String::from_utf8_lossy(&outcome.stdout);
        assert!(stdout.contains("yellow-gate"));
    }

    /// Red gate: spawn must be refused with an explicit, actionable error
    /// after THERMAL_MAX_RETRIES attempts.  The error message must mention
    /// "thermally throttled" so the operator can act on it.
    #[tokio::test(start_paused = true)]
    async fn thermal_gate_red_refuses_spawn_with_loud_error() {
        let dir = TempDir::new().expect("tempdir");
        let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Refuse));
        let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

        let req = SpawnRequest {
            argv: echo_argv("red-gate"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        let result = hv.run(req).await;
        assert!(result.is_err(), "Red gate must refuse spawn");

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("thermally throttled"),
            "error must mention 'thermally throttled', got: {msg}"
        );
        assert!(
            msg.contains(&THERMAL_MAX_RETRIES.to_string()),
            "error must mention retry count, got: {msg}"
        );
    }

    /// A gate that transitions Green → Red → Green validates that the retry
    /// loop recovers once the device cools down.
    ///
    /// The gate starts RED for the first call, then returns Green on the second
    /// call — simulating thermal recovery after one sleep-retry.
    #[tokio::test(start_paused = true)]
    async fn thermal_gate_recovers_after_one_red_attempt() {
        /// A gate that returns Refuse on the first check, then Allow forever.
        struct OneRedThenGreen {
            calls: AtomicU32,
        }
        impl ThermalGate for OneRedThenGreen {
            fn check(&self) -> ThermalDecision {
                let n = self.calls.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    ThermalDecision::Refuse
                } else {
                    ThermalDecision::Allow
                }
            }
        }

        let dir = TempDir::new().expect("tempdir");
        let gate = Arc::new(OneRedThenGreen { calls: AtomicU32::new(0) });
        let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

        let req = SpawnRequest {
            argv: echo_argv("recover"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        // With `start_paused` tokio::time::sleep resolves immediately in tests.
        let outcome = hv.run(req).await.expect("should succeed after one RED retry");
        assert_eq!(outcome.exit_code, 0);
        let stdout = String::from_utf8_lossy(&outcome.stdout);
        assert!(stdout.contains("recover"));
    }

    // ── FUSE IO-intercept tests ──────────────────────────────────────────

    /// FuseGuard::try_mount never panics and returns a valid object on all
    /// platforms.  On non-FUSE platforms the guard is a no-op; on Linux/macOS
    /// it may or may not succeed depending on the FUSE kernel module. When the
    /// mount is not ready, mountpoint() MUST be None (never an unready Some).
    #[test]
    fn fuse_guard_try_mount_never_panics() {
        let dir = TempDir::new().expect("tempdir");
        let guard = FuseGuard::try_mount(dir.path(), "hv-test-session");
        // The guard object is valid regardless of whether FUSE is active.
        // Dropping it must not panic.
        drop(guard);
    }

    /// When FUSE is active the mountpoint must differ from the backing path
    /// and the readiness marker must be readable through it. When FUSE is
    /// inactive mountpoint() returns None — the no-op guard.
    #[test]
    fn fuse_guard_mountpoint_returns_path_or_none() {
        let dir = TempDir::new().expect("tempdir");
        let guard = FuseGuard::try_mount(dir.path(), "hv-test-session");
        match guard.mountpoint() {
            Some(mp) => {
                // FUSE is active — mountpoint must be different from backing.
                assert_ne!(mp, dir.path(), "mountpoint must differ from backing");
                assert!(mp.starts_with(std::env::temp_dir()), "mountpoint must be under temp dir");
                let probe_found = std::fs::read_dir(mp)
                    .expect("read mountpoint")
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().starts_with(FUSE_READY_MARKER));
                assert!(
                    probe_found,
                    "readiness marker must be visible through mountpoint before Some is returned"
                );
            }
            None => {
                // FUSE not available — best-effort, still valid.
            }
        }
    }

    /// FR-009 / AC-009.14 — FuseGuard stays mounted until drop (spawn/teardown lifecycle).
    #[test]
    fn fuse_guard_teardown_after_drop() {
        let dir = TempDir::new().expect("tempdir");
        let mountpoint = {
            let guard = FuseGuard::try_mount(dir.path(), "hv-teardown-session");
            guard.mountpoint().map(|p| p.to_path_buf())
        };
        if let Some(mp) = mountpoint {
            assert_ne!(mp, dir.path());
            // Guard drop runs force-unmount; mount tempdir may remain but must not
            // still expose the readiness marker tree as a live FUSE mount.
            let still_mounted = std::fs::read_dir(&mp).ok().is_some_and(|mut rd| {
                rd.any(|e| {
                    e.ok().is_some_and(|ent| {
                        ent.file_name().to_string_lossy().starts_with(FUSE_READY_MARKER)
                    })
                })
            });
            assert!(!still_mounted, "FUSE intercept MUST tear down on guard drop (AC-009.14)");
        }
    }

    /// Readiness poll reports mount failure immediately when fail_rx fires.
    #[test]
    fn fuse_ready_poll_fails_loudly_on_mount_error() {
        let (tx, rx) = mpsc::channel();
        tx.send("simulated mount boom".into()).expect("send");
        let probe = Path::new("/tmp/sharecli-fuse-ready-poll-never");
        let err =
            wait_fuse_mount_ready(probe, &rx, Duration::from_secs(2), Duration::from_millis(10))
                .expect_err("must fail when mount reports error");
        assert!(err.contains("mount failed"), "loud error must mention mount failure: {err}");
        assert!(
            err.contains("simulated mount boom"),
            "loud error must include mount message: {err}"
        );
    }

    /// Readiness poll times out with a loud message when the probe never appears.
    #[test]
    fn fuse_ready_poll_times_out_loudly_when_probe_missing() {
        let (_tx, rx) = mpsc::channel::<String>();
        let probe = Path::new("/tmp/sharecli-fuse-ready-poll-missing-xyz");
        let err =
            wait_fuse_mount_ready(probe, &rx, Duration::from_millis(80), Duration::from_millis(20))
                .expect_err("must time out when probe never appears");
        assert!(err.contains("timed out"), "loud error must mention timeout: {err}");
        assert!(err.contains("sharecli-fuse"), "loud error must identify sharecli-fuse: {err}");
    }

    /// Readiness poll succeeds when the probe file becomes readable.
    #[test]
    fn fuse_ready_poll_succeeds_when_probe_appears() {
        let dir = TempDir::new().expect("tempdir");
        let probe = dir.path().join("appears.txt");
        let (_tx, rx) = mpsc::channel::<String>();
        let probe_bg = probe.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            std::fs::write(&probe_bg, b"ready").expect("write probe");
        });
        wait_fuse_mount_ready(&probe, &rx, Duration::from_secs(2), Duration::from_millis(10))
            .expect("probe must become ready");
    }

    /// AC-009.12 — FUSE session id is deterministic and keyed from CommandKey prefix.
    #[test]
    fn fuse_session_id_for_command_key_is_deterministic() {
        let key = command_key(&["cargo".into(), "build".into()], Path::new("/repo"), &[]);
        let a = fuse_session_id_for_command_key(&key);
        let b = fuse_session_id_for_command_key(&key);
        assert_eq!(a, b);
        assert!(a.starts_with("hv-"));
        assert_eq!(a.len(), 19);
    }

    /// HypervisorConfig.coalesce_debounce is plumbed into CoalesceCache so every
    /// Hypervisor::run coalesce path uses with_lock's debounce re-check (AC-008.6).
    #[tokio::test]
    async fn hypervisor_coalesce_debounce_wired_from_config() {
        let dir = TempDir::new().expect("tempdir");
        let debounce = Duration::from_millis(75);
        let hv = Hypervisor::with_options(
            HypervisorConfig {
                cache_root: dir.path().join("cache"),
                queue_root: dir.path().join("queue"),
                queue_max_concurrent: 1,
                coalesce_ttl: CoalesceCache::DEFAULT_TTL,
                coalesce_debounce: debounce,
                cache_key_mode: CacheKeyMode::Time,
                semantic: false,
            },
            Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
            vec![],
        );
        assert_eq!(
            hv.coalesce_debounce(),
            debounce,
            "AC-008.6: Hypervisor coalesce path MUST carry config debounce"
        );
    }

    /// Default Hypervisor constructors leave debounce disabled (opt-in via config).
    #[tokio::test]
    async fn hypervisor_default_debounce_is_zero() {
        let dir = TempDir::new().expect("tempdir");
        let hv = Hypervisor::new(dir.path());
        assert_eq!(hv.coalesce_debounce(), Duration::ZERO);
    }

    /// FR-008 / AC-008.16 — `run_queued` never serves coalesce cache.
    #[tokio::test]
    async fn hypervisor_run_queued_skips_coalesce_cache() {
        let dir = TempDir::new().expect("tempdir");
        let hv = allow_hypervisor(dir.path());
        let req = SpawnRequest::from_operator(
            echo_argv("queued-twice"),
            dir.path().to_path_buf(),
            vec![],
            None,
        );

        let first = hv.run_queued(req.clone(), "echo").await.expect("first run_queued");
        assert!(!first.from_cache, "AC-008.16: run_queued MUST NOT use cache on first run");

        let second = hv.run_queued(req, "echo").await.expect("second run_queued");
        assert!(!second.from_cache, "AC-008.16: run_queued MUST NOT use cache on replay");
        assert_eq!(second.exit_code, 0);
    }

    /// With FUSE wired into the run() path, a spawn must still succeed
    /// regardless of whether the intercept is active.
    #[tokio::test]
    async fn fuse_io_run_succeeds_with_or_without_intercept() {
        let dir = TempDir::new().expect("tempdir");
        let hv = allow_hypervisor(dir.path());

        let req = SpawnRequest {
            argv: echo_argv("fuse-run"),
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        };

        let outcome = hv.run(req).await.expect("run must succeed with fuse-io wiring");
        assert_eq!(outcome.exit_code, 0);
        assert!(!outcome.from_cache);
        let stdout = String::from_utf8_lossy(&outcome.stdout);
        assert!(stdout.contains("fuse-run"));
    }

    /// FakeThermalGate::check() must return the decision it was constructed with.
    #[test]
    fn fake_thermal_gate_returns_configured_decision() {
        for decision in [ThermalDecision::Allow, ThermalDecision::Warn, ThermalDecision::Refuse] {
            let gate = FakeThermalGate::new(decision);
            assert_eq!(gate.check(), decision);
        }
    }

    /// ThermalDecision variants are Copy + Eq — spot-check the impls.
    #[test]
    fn thermal_decision_copy_eq() {
        let a = ThermalDecision::Allow;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(ThermalDecision::Allow, ThermalDecision::Refuse);
    }

    #[test]
    fn combine_thermal_agent_decision_escalates_allow_to_warn() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Warn),
            ThermalDecision::Warn
        );
    }

    #[test]
    fn combine_thermal_agent_decision_agent_refuse_overrides_allow() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Refuse),
            ThermalDecision::Refuse
        );
    }

    /// FR-011 / AC-011.4 — high agent inventory refuses spawn even when thermal Allows.
    #[tokio::test(start_paused = true)]
    async fn agent_aware_gate_refuses_at_contention_limit() {
        fn eight_agents() -> usize {
            8
        }
        let dir = TempDir::new().expect("tempdir");
        let inner = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
        let gate = Arc::new(AgentAwareThermalGate::with_agent_count(
            inner,
            AgentContentionThresholds::default(),
            eight_agents,
        ));
        let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

        let err = hv
            .run(SpawnRequest {
                argv: echo_argv("gated"),
                cwd: dir.path().to_path_buf(),
                env: vec![],
                queue_priority: QueuePriority::Normal,
            })
            .await
            .expect_err("agent contention Refuse MUST err after retries");

        let msg = err.to_string();
        assert!(
            msg.contains("thermally throttled"),
            "error must mention thermally throttled, got: {msg}"
        );
    }

    // ── New tests: SpawnRequest, SpawnOutcome, CachedResult, edge cases ──────

    /// SpawnRequest::new wires queue_priority to Normal (default operator path).
    #[test]
    fn spawn_request_new_defaults_to_normal_priority() {
        let req =
            SpawnRequest::new(vec!["cargo".into(), "build".into()], PathBuf::from("/repo"), vec![]);
        assert_eq!(req.queue_priority, QueuePriority::Normal);
        assert_eq!(req.argv, vec!["cargo", "build"]);
        assert_eq!(req.cwd, PathBuf::from("/repo"));
        assert!(req.env.is_empty());
    }

    /// SpawnRequest::with_queue_priority overrides the priority field.
    #[test]
    fn spawn_request_with_queue_priority_overrides() {
        let req = SpawnRequest::new(vec!["echo".into()], PathBuf::from("."), vec![])
            .with_queue_priority(QueuePriority::High);
        assert_eq!(req.queue_priority, QueuePriority::High);
    }

    /// SpawnOutcome::agent_family returns None when detected_agent is None.
    #[test]
    fn spawn_outcome_agent_family_none_when_no_agent() {
        let outcome = SpawnOutcome {
            exit_code: 0,
            stdout: vec![],
            stderr: vec![],
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        };
        assert_eq!(outcome.agent_family(), None);
    }

    /// SpawnOutcome::fuse_intercept_active returns false when no FUSE session.
    #[test]
    fn spawn_outcome_fuse_intercept_inactive_without_session() {
        let outcome = SpawnOutcome {
            exit_code: 0,
            stdout: vec![],
            stderr: vec![],
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        };
        assert!(!outcome.fuse_intercept_active());
    }

    /// SpawnOutcome::fuse_intercept_active returns true when session is set.
    #[test]
    fn spawn_outcome_fuse_intercept_active_with_session() {
        let outcome = SpawnOutcome {
            exit_code: 0,
            stdout: vec![],
            stderr: vec![],
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: Some("hv-abcdef0123456789".into()),
            fuse_backing: Some(PathBuf::from("/repo")),
            fuse_mountpoint: Some(PathBuf::from("/tmp/fuse")),
        };
        assert!(outcome.fuse_intercept_active());
    }

    /// SpawnOutcome::remap_fuse_path returns None when FUSE is inactive.
    #[test]
    fn spawn_outcome_remap_fuse_path_none_without_intercept() {
        let outcome = SpawnOutcome {
            exit_code: 0,
            stdout: vec![],
            stderr: vec![],
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        };
        assert_eq!(outcome.remap_fuse_path(Path::new("/some/path")), None);
    }

    /// CachedResult -> SpawnOutcome conversion preserves data and sets from_cache=true.
    #[test]
    fn cached_result_to_spawn_outcome_preserves_fields() {
        let cached =
            CachedResult { exit_code: 42, stdout: b"hello".to_vec(), stderr: b"err".to_vec() };
        let outcome: SpawnOutcome = cached.into();
        assert_eq!(outcome.exit_code, 42);
        assert_eq!(outcome.stdout, b"hello");
        assert_eq!(outcome.stderr, b"err");
        assert!(outcome.from_cache, "From<CachedResult> must set from_cache=true");
        assert!(outcome.detected_agent.is_none());
        assert!(outcome.fuse_session_id.is_none());
    }

    /// SpawnOutcome -> CachedResult conversion preserves exit_code, stdout, stderr.
    #[test]
    fn spawn_outcome_to_cached_result_preserves_output() {
        let outcome = SpawnOutcome {
            exit_code: 7,
            stdout: b"out".to_vec(),
            stderr: vec![],
            from_cache: false,
            resource_watch: ResourceWatchSample::default(),
            detected_agent: None,
            fuse_session_id: None,
            fuse_backing: None,
            fuse_mountpoint: None,
        };
        let cached: CachedResult = outcome.into();
        assert_eq!(cached.exit_code, 7);
        assert_eq!(cached.stdout, b"out");
        assert!(cached.stderr.is_empty());
    }

    /// fuse_session_id_for_command_key truncates short keys gracefully.
    #[test]
    fn fuse_session_id_for_short_command_key() {
        // Key shorter than 16 chars — prefix should be the full key.
        let key = CommandKey("abc".into());
        let id = fuse_session_id_for_command_key(&key);
        assert!(id.starts_with("hv-"));
        assert_eq!(id, "hv-abc");
    }

    /// fuse_session_id_for_command_key uses exactly 16 chars from the key.
    #[test]
    fn fuse_session_id_uses_exactly_16_char_prefix() {
        let key = CommandKey("0123456789abcdef12345678".into());
        let id = fuse_session_id_for_command_key(&key);
        assert_eq!(id, "hv-0123456789abcdef");
        assert_eq!(id.len(), 19); // "hv-" + 16 hex chars
    }

    /// combine_thermal_agent_decision: Warn thermal + AgentsOk stays Warn.
    #[test]
    fn combine_thermal_agent_decision_warn_agents_ok_stays_warn() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Warn, AgentContentionTier::Ok),
            ThermalDecision::Warn
        );
    }

    /// combine_thermal_agent_decision: Warn thermal + AgentsRefuse -> Refuse.
    #[test]
    fn combine_thermal_agent_decision_warn_agents_refuse_escalates() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Warn, AgentContentionTier::Refuse),
            ThermalDecision::Refuse
        );
    }

    /// queue_lane_from_argv: extracts basename from argv[0].
    #[test]
    fn queue_lane_from_argv_extracts_basename() {
        let argv = vec!["/usr/local/bin/cargo".into()];
        assert_eq!(queue_lane_from_argv(&argv), "cargo");
    }

    /// queue_lane_from_argv: returns "unknown" for empty argv.
    #[test]
    fn queue_lane_from_argv_empty_returns_unknown() {
        let argv: Vec<String> = vec![];
        assert_eq!(queue_lane_from_argv(&argv), "unknown");
    }

    /// Hypervisor accessors: coalesce_ttl, queue_max_concurrent, nocache_args.
    #[test]
    fn hypervisor_accessor_methods_return_config_values() {
        let dir = TempDir::new().expect("tempdir");
        let hv = allow_hypervisor(dir.path());
        assert_eq!(hv.coalesce_ttl(), CoalesceCache::DEFAULT_TTL);
        assert_eq!(hv.queue_max_concurrent(), 1);
        assert!(!hv.nocache_args().is_empty(), "default nocache_args should not be empty");
    }

    /// Hypervisor::set_nocache_args replaces the flag list.
    #[test]
    fn hypervisor_set_nocache_args_replaces_flags() {
        let dir = TempDir::new().expect("tempdir");
        let mut hv = allow_hypervisor(dir.path());
        let original_len = hv.nocache_args().len();
        hv.set_nocache_args(vec!["--force".into(), "--clean".into()]);
        assert_eq!(hv.nocache_args(), &["--force", "--clean"]);
        assert_ne!(hv.nocache_args().len(), original_len);
    }
}
