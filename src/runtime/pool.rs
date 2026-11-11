//! `ProcessPool` — the metadata+handle pool that owns spawn / kill / list /
//! find operations. The actual process control is delegated to a
//! `runtime_process::CommandGroupProcess` (substrate `ProcessPort` impl).
//!
//! The pool is paired with an optional `SpawnPolicy` so build harnesses
//! (cargo / rustc / make / …) can be throttled via a build-permit semaphore,
//! get `taskpolicy -b` wrapping on macOS, and receive `CARGO_BUILD_JOBS`
//! / `RUSTC_WRAPPER=sccache` env-var injection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use runtime_process::CommandGroupProcess;
use substrate::{ProcessHandle, ProcessPort, ProcessSpawnSpec};
use sysinfo::Pid;
use tokio::sync::RwLock;

use crate::spawn_policy::{is_build_harness, SpawnPolicy};

use super::audit::ProcState;
use super::audit::{emit_spawn_audit, emit_stop_audit, spawn_capability};
use super::managed::ManagedProcess;
use super::process_info::{count_open_fds, count_threads, EnvGuard, ProcessInfo};

/// Metadata + handle pool for processes spawned via the substrate
/// `ProcessPort`. All public surface (new / with_spawn_policy / refresh /
/// list / spawn / kill / kill_all / system_memory_usage / find) is async.
///
/// `processes` and `system` are `pub(crate)` so the unit tests in
/// `runtime::tests` can inject fixtures (via `insert_unowned_handle`) and
/// inspect the pool's internal state without going through the public API.
pub struct ProcessPool {
    pub(crate) processes: RwLock<HashMap<u32, ManagedProcess>>,
    pub(crate) system: RwLock<sysinfo::System>,
    port: CommandGroupProcess,
    /// Optional spawn-policy throttle. When `Some`, build harnesses (cargo/rustc/…)
    /// must acquire a permit before spawning, and receive injected env-vars and
    /// optional taskpolicy wrapping.
    spawn_policy: Option<Arc<SpawnPolicy>>,
}

impl Default for ProcessPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessPool {
    pub fn new() -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
            system: RwLock::new(sysinfo::System::new_all()),
            port: CommandGroupProcess::new(),
            spawn_policy: None,
        }
    }

    /// Create a `ProcessPool` with a build-contention throttle applied to
    /// cargo/rustc and other build harnesses.
    pub fn with_spawn_policy(policy: Arc<SpawnPolicy>) -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
            system: RwLock::new(sysinfo::System::new_all()),
            port: CommandGroupProcess::new(),
            spawn_policy: Some(policy),
        }
    }

    /// Refresh system process information
    pub async fn refresh(&self) {
        let mut sys = self.system.write().await;
        sys.refresh_all();
    }

    /// Get all managed processes
    pub async fn list(&self) -> Vec<ProcessInfo> {
        let sys = self.system.read().await;
        let procs = self.processes.read().await;

        let mut result = Vec::new();
        for (pid, managed) in procs.iter() {
            if let Some(mut info) =
                ProcessInfo::from_sysinfo(Pid::from_u32(*pid), managed.info.name.clone(), &sys)
            {
                info.project = managed.info.project.clone();
                info.harness = managed.info.harness.clone();
                result.push(info);
            }
        }
        result
    }

    /// Spawn a new process via substrate ProcessPort.
    ///
    /// When a `SpawnPolicy` is configured and the harness is a recognised build
    /// harness (cargo/rustc/make/…), this method:
    /// 1. Acquires a semaphore permit — queuing if `max_concurrent_builds` slots
    ///    are already taken (released automatically when the permit is dropped at
    ///    the end of this call; callers that want to hold the slot for the full
    ///    build duration should use `ProcessPool::with_spawn_policy` and
    ///    `SpawnPolicy::acquire_build_permit` directly).
    /// 2. Wraps the command with `taskpolicy -b` on macOS when `nice_level > 0`.
    /// 3. Injects `CARGO_BUILD_JOBS` and optionally `RUSTC_WRAPPER=sccache`.
    pub async fn spawn(
        &self,
        cmd: &str,
        args: &[String],
        cwd: Option<PathBuf>,
        project: Option<String>,
        harness: Option<String>,
    ) -> Result<ProcessInfo> {
        // Determine whether the spawn-policy throttle applies to this harness.
        let is_build =
            harness.as_deref().map(is_build_harness).unwrap_or_else(|| is_build_harness(cmd));

        // Apply policy when present and harness is a build harness.
        let _permit;
        let (effective_cmd, effective_args, extra_env): (
            String,
            Vec<String>,
            Vec<(String, String)>,
        ) = if is_build {
            if let Some(ref policy) = self.spawn_policy {
                // Acquire build slot (queues if at cap).
                _permit = Some(policy.acquire_build_permit().await?);
                let (prog, shaped_args) = policy.apply_taskpolicy(cmd, args);
                let env = policy.build_env_overrides();
                (prog, shaped_args, env)
            } else {
                _permit = None;
                (cmd.to_string(), args.to_vec(), vec![])
            }
        } else {
            _permit = None;
            (cmd.to_string(), args.to_vec(), vec![])
        };

        let mut extra_env = extra_env;
        if let Some(pair) = crate::otel::traceparent_spawn_env() {
            extra_env.push(pair);
        }

        // Propagate env-var overrides into the child environment before spawn.
        // substrate's `ProcessSpawnSpec` does not yet carry env — apply scoped overrides
        // on a blocking thread so we never hold `set_var` across async yield points
        // (C00 L4 — see `docs/ops/async-shutdown.md`).
        let spec =
            ProcessSpawnSpec { program: effective_cmd.clone(), args: effective_args.clone(), cwd };
        let port = self.port.clone();
        let capability = spawn_capability(cmd, &harness);
        let spawn_program = effective_cmd.clone();
        let handle = match tokio::task::spawn_blocking(move || -> anyhow::Result<ProcessHandle> {
            let _env_guards: Vec<EnvGuard> = extra_env
                .into_iter()
                .map(|(k, v)| {
                    let prev = std::env::var(&k).ok();
                    // SAFETY: env mutation is serialized to this blocking thread only.
                    unsafe { std::env::set_var(&k, &v) };
                    EnvGuard { key: k, prev }
                })
                .collect();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow::anyhow!("spawn runtime: {e}"))?;
            rt.block_on(async {
                port.spawn(&spec).await.map_err(|e| anyhow::anyhow!("spawn {}: {e}", spec.program))
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking join: {e}"))?
        {
            Ok(handle) => handle,
            Err(e) => {
                emit_spawn_audit(&project, &capability, "denied", None);
                return Err(anyhow!("spawn {spawn_program}: {e}"));
            }
        };

        let pid = handle.pid;

        self.refresh().await;

        let info = ProcessInfo {
            pid,
            name: effective_cmd.clone(),
            cmd: vec![effective_cmd].into_iter().chain(effective_args).collect(),
            memory_mb: 0,
            start_time: 0,
            cpu_percent: 0.0,
            project,
            harness,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::Unknown,
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        };

        let managed = ManagedProcess { info: info.clone(), handle };

        let mut procs = self.processes.write().await;
        procs.insert(pid, managed);

        emit_spawn_audit(&info.project, &capability, "ok", Some(pid));

        Ok(info)
    }

    /// Kill a process by PID via substrate ProcessPort
    pub async fn kill(&self, pid: u32) -> Result<()> {
        let mut procs = self.processes.write().await;
        if let Some(managed) = procs.get(&pid) {
            let capability = spawn_capability(&managed.info.name, &managed.info.harness);
            match self.port.kill_group(&managed.handle).await {
                Ok(()) => {
                    emit_stop_audit(&managed.info.project, &capability, pid, "ok");
                }
                Err(e) => {
                    emit_stop_audit(&managed.info.project, &capability, pid, "denied");
                    return Err(anyhow!("kill pid {pid}: {e}"));
                }
            }
            procs.remove(&pid);
        }
        Ok(())
    }

    /// Kill all managed processes
    pub async fn kill_all(&self) -> Result<()> {
        let mut procs = self.processes.write().await;
        let mut stopped = Vec::new();
        let mut failures = Vec::new();
        for (&pid, managed) in procs.iter() {
            let capability = spawn_capability(&managed.info.name, &managed.info.harness);
            let outcome = match self.port.kill_group(&managed.handle).await {
                Ok(()) => {
                    stopped.push(pid);
                    "ok"
                }
                Err(error) => {
                    failures.push(format!("kill pid {pid}: {error}"));
                    "denied"
                }
            };
            emit_stop_audit(&managed.info.project, &capability, pid, outcome);
        }
        for pid in stopped {
            procs.remove(&pid);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(failures.join("; ")))
        }
    }

    /// Get system memory usage
    pub async fn system_memory_usage(&self) -> (u64, u64) {
        let sys = self.system.read().await;
        (sys.used_memory() / 1024 / 1024, sys.total_memory() / 1024 / 1024)
    }

    /// Find managed processes matching `filter`. Refreshes sysinfo first so
    /// freshly-spawned PIDs that are no longer alive are filtered out.
    pub async fn find(&self, filter: ProcessFilter) -> Vec<ProcessInfo> {
        self.refresh().await;
        let sys = self.system.read().await;
        let procs = self.processes.read().await;

        let mut result = Vec::new();

        for (pid, managed) in procs.iter() {
            let info =
                ProcessInfo::from_sysinfo(Pid::from_u32(*pid), managed.info.name.clone(), &sys);

            if let Some(mut info) = info {
                info.project = managed.info.project.clone();
                info.harness = managed.info.harness.clone();
                let matches = match &filter {
                    ProcessFilter::All => true,
                    ProcessFilter::ByProject(proj) => info.project.as_ref() == Some(proj),
                    ProcessFilter::ByHarness(harness) => info.harness.as_ref() == Some(harness),
                };

                if matches {
                    result.push(info);
                }
            }
        }

        result
    }
}

/// Filter for `ProcessPool::find` — narrow the result set to one project or
/// harness, or accept everything.
#[derive(Debug, Clone)]
pub enum ProcessFilter {
    All,
    ByProject(String),
    ByHarness(String),
}

// Reference the helpers so the compiler doesn't flag them unused; `count_open_fds`
// and `count_threads` are called from `ProcessInfo::from_sysinfo` (in
// `process_info.rs`) and re-anchored here for unit-test discoverability.
#[allow(unused_imports)]
use {count_open_fds as _count_open_fds_for_anchor, count_threads as _count_threads_for_anchor};
