// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Public types — `HypervisorConfig`, `SpawnRequest`, `SpawnOutcome`,
//! and the `CachedResult` ↔ `SpawnOutcome` conversions required by
//! `CoalesceCache::with_lock`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sharecli_fleet::ResourceWatchSample;
use sharecli_fuse::remap_mount_to_backing;
use sharecli_ipc::{resolve_operator_queue_priority, CacheKeyMode, CachedResult, QueuePriority};

use sharecli_fleet::proc_scan::DetectedAgent;

// ---------------------------------------------------------------------------
// HypervisorConfig
// ---------------------------------------------------------------------------

/// Configuration for the [`crate::Hypervisor`].
#[derive(Debug, Clone)]
pub struct HypervisorConfig {
    /// Root directory for the coalesce cache.
    pub cache_root: PathBuf,
    /// Root directory for the mutating/nocache [`sharecli_ipc::SlotQueue`].
    pub queue_root: PathBuf,
    /// Max parallel slots for queued (mutating) commands.
    pub queue_max_concurrent: usize,
    /// TTL for coalesce cache entries (origin harness rules.conf `ttl=`).
    pub coalesce_ttl: Duration,
    /// Debounce window for the coalesce miss path (origin harness `debounce_ms`).
    ///
    /// When non-zero, [`sharecli_ipc::CoalesceCache::with_lock`] waits then re-checks so an
    /// in-window sibling store is shared (AC-008.6). [`Duration::ZERO`] disables.
    pub coalesce_debounce: Duration,
    /// Cache-key mode from rules.conf `cache_key=` (AC-008.19).
    pub cache_key_mode: CacheKeyMode,
    /// When true, apply lint-tool semantic argv normalization before hashing (AC-008.20).
    pub semantic: bool,
}

// ---------------------------------------------------------------------------
// SpawnRequest
// ---------------------------------------------------------------------------

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
    /// Non-empty [`sharecli_ipc::QUEUE_PRIORITY_ENV`] wins over `rule_priority`
    /// (FR-008 AC-008.15).
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

// ---------------------------------------------------------------------------
// SpawnOutcome
// ---------------------------------------------------------------------------

/// The outcome of a [`crate::Hypervisor::run`] call.
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
    /// Live FD/net watch sample captured at [`crate::Hypervisor::run`] entry (FR-007).
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
    /// `Some` only when [`fuse_intercept_active`](Self::fuse_intercept_active) is
    /// true; pairs with [`fuse_mountpoint`](Self::fuse_mountpoint) for
    /// [`remap_fuse_path`](Self::remap_fuse_path).
    pub fuse_backing: Option<PathBuf>,
    /// Ephemeral FUSE mountpoint used as the child cwd (AC-009.14).
    pub fuse_mountpoint: Option<PathBuf>,
}

impl SpawnOutcome {
    pub(crate) fn with_resource_watch(mut self, watch: ResourceWatchSample) -> Self {
        self.resource_watch = watch;
        self
    }

    pub(crate) fn with_detected_agent(mut self, agent: Option<DetectedAgent>) -> Self {
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
            (Some(mp), Some(bk)) => remap_mount_to_backing(mp, bk, path),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CachedResult ↔ SpawnOutcome conversions
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
// Type unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use sharecli_fleet::ResourceWatchSample;
    use sharecli_ipc::{CachedResult, QueuePriority};

    use super::{HypervisorConfig, SpawnOutcome, SpawnRequest};

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

    /// HypervisorConfig holds all config fields and clones identically.
    #[test]
    fn hypervisor_config_clone_preserves_all_fields() {
        let cfg = HypervisorConfig {
            cache_root: PathBuf::from("/cache"),
            queue_root: PathBuf::from("/queue"),
            queue_max_concurrent: 4,
            coalesce_ttl: Duration::from_secs(60),
            coalesce_debounce: Duration::from_millis(50),
            cache_key_mode: sharecli_ipc::CacheKeyMode::Time,
            semantic: true,
        };
        let cloned = cfg.clone();
        assert_eq!(cloned.cache_root, PathBuf::from("/cache"));
        assert_eq!(cloned.queue_root, PathBuf::from("/queue"));
        assert_eq!(cloned.queue_max_concurrent, 4);
        assert_eq!(cloned.coalesce_ttl, Duration::from_secs(60));
        assert_eq!(cloned.coalesce_debounce, Duration::from_millis(50));
        assert!(cloned.semantic);
    }
}
