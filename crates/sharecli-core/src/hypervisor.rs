//! The sharecli hypervisor engine.
//!
//! Owns a [`CoalesceCache`] and routes every [`SpawnRequest`] through the
//! Lock-Wait-Cache protocol: identical concurrent commands coalesce into a
//! single execution, with all waiters receiving the same cached result.
//!
//! A [`ThermalGate`] is consulted before every spawn.  When the device is in a
//! RED thermal state the hypervisor enters a visible sleep-retry loop and, if
//! the state does not clear within [`THERMAL_MAX_RETRIES`] attempts, returns an
//! explicit error rather than silently dropping or degrading the spawn.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use sharecli_fleet::agent_contention::AgentContentionThresholds;
use sharecli_fleet::ResourceWatchSample;
use sharecli_ipc::{
    command_key_with_mode, has_nocache_arg, record_coalesce_lookup_hit, record_nocache_run,
    semantic_normalize_argv, CacheKeyMode, CachedResult, CoalesceCache, CoalesceHitKind,
    CommandKey, SlotQueue, DEFAULT_NOCACHE_ARGS,
};
use tracing::debug;

use crate::fuse_guard::{fuse_session_id_for_command_key, FuseGuard};
use crate::proc_scan::detect_caller_agent;
use crate::speculation::SpeculationTracker;
use crate::thermal_gate::{
    AgentAwareThermalGate, SystemThermalGate, ThermalDecision, ThermalGate, THERMAL_MAX_RETRIES,
    THERMAL_RETRY_SLEEP,
};
use crate::types::{HypervisorConfig, SpawnOutcome, SpawnRequest};

// ---------------------------------------------------------------------------
// Hypervisor
// ---------------------------------------------------------------------------

/// The sharecli hypervisor engine.
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
    /// Intended for tests that inject a [`FakeThermalGate`](crate::thermal_gate::FakeThermalGate)
    /// (or any other implementation) to exercise gate behaviour without real hardware.
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
        crate::speculation::spawn_speculation_task(
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
        key: &CommandKey,
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
                    tracing::warn!("thermal-gate: Yellow — device is warm, proceeding with spawn");
                    return Ok(());
                }
                ThermalDecision::Refuse => {
                    attempt += 1;
                    // Loud, actionable message — never a silent no-op.
                    eprintln!(
                        "sharecli: Waiting for thermal headroom\u{2026} ({attempt}/{THERMAL_MAX_RETRIES})"
                    );
                    tracing::warn!(
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
pub(crate) fn queue_lane_from_argv(argv: &[String]) -> &str {
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
