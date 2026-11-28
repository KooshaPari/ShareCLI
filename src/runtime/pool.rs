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
use super::registry::{
    entry_is_live, identity_matches_live, pid_alive, signal_group_or_pid, Registry, RegistryEntry,
};

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
    /// Registry file this pool records/reads managed pids in. `None` uses the
    /// device registry (`Registry::open()`); tests inject a private path so they
    /// never observe or mutate the operator's real device registry.
    registry_path: Option<PathBuf>,
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
            registry_path: None,
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
            registry_path: None,
        }
    }

    /// Test-only: a pool whose process registry is a private file, so it never
    /// reads or writes the device-wide registry shared with other invocations.
    #[cfg(test)]
    pub(crate) fn with_registry_path(path: PathBuf) -> Self {
        Self { registry_path: Some(path), ..Self::new() }
    }

    /// The registry this pool records managed pids in.
    fn registry(&self) -> Registry {
        match &self.registry_path {
            Some(path) => Registry::at(path.clone()),
            None => Registry::open(),
        }
    }

    /// Refresh system process information
    pub async fn refresh(&self) {
        let mut sys = self.system.write().await;
        sys.refresh_all();
    }

    /// Get all managed processes
    pub async fn list(&self) -> Vec<ProcessInfo> {
        // Resolve the device registry BEFORE taking any lock: the registry read
        // and liveness probes are synchronous file I/O, and no synchronous
        // filesystem call may run while an async `RwLock` guard is held. `list`
        // is a read path, so it must not prune or otherwise rewrite the file.
        let recorded = self.registry().live_entries();

        let mut result = Vec::new();
        {
            let sys = self.system.read().await;
            let procs = self.processes.read().await;
            for (pid, managed) in procs.iter() {
                if let Some(mut info) =
                    ProcessInfo::from_sysinfo(Pid::from_u32(*pid), managed.info.name.clone(), &sys)
                {
                    info.project = managed.info.project.clone();
                    info.harness = managed.info.harness.clone();
                    result.push(info);
                }
            }
        }

        if !recorded.is_empty() {
            let seen: std::collections::HashSet<u32> = result.iter().map(|p| p.pid).collect();
            let sys = self.system.read().await;
            // Processes recorded by earlier invocations: the device authority.
            for entry in recorded {
                if seen.contains(&entry.pid) {
                    continue;
                }
                let name = entry.cmd.first().cloned().unwrap_or_default();
                if let Some(mut info) =
                    ProcessInfo::from_sysinfo(Pid::from_u32(entry.pid), name, &sys)
                {
                    info.project = entry.project.clone();
                    info.harness = entry.harness.clone();
                    result.push(info);
                }
            }
        }
        result
    }

    /// Spawn a new process via substrate ProcessPort.
    ///
    /// When a `SpawnPolicy` is configured and the harness is a recognised build
    /// harness (cargo/rustc/make/…), this method:
    /// 1. Takes a device-wide build slot — queuing if `max_concurrent_builds`
    ///    slots are already held by this or any sibling `sharecli` invocation.
    ///    The slot is handed to an out-of-process supervisor for the direct
    ///    build pid, so it is released when the build exits, not when this call
    ///    returns or when the spawning process exits.
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
        //
        // Admission is device-scoped: a build slot is flocked under the device
        // state directory and the descriptor is inherited by the child, so the
        // slot is held for the whole build even though `sharecli start` exits
        // immediately after spawning. The in-process semaphore is only a
        // fallback for when the state directory is not writable, because it
        // cannot see a sibling invocation.
        let mut lease = None;
        let mut permit = None;
        let (effective_cmd, effective_args, extra_env): (
            String,
            Vec<String>,
            Vec<(String, String)>,
        ) = if is_build {
            if let Some(ref policy) = self.spawn_policy {
                let slots = policy.config.max_concurrent_builds.max(1);
                match super::admission::SlotPool::device(slots).acquire().await {
                    Ok(acquired) => lease = Some(acquired),
                    Err(error) => {
                        eprintln!(
                            "sharecli: warning: device build admission unavailable ({error}); \
                             admitting within this process only"
                        );
                        permit = Some(policy.acquire_build_permit().await?);
                    }
                }
                let (prog, shaped_args) = policy.apply_taskpolicy(cmd, args);
                let env = policy.build_env_overrides();
                (prog, shaped_args, env)
            } else {
                (cmd.to_string(), args.to_vec(), vec![])
            }
        } else {
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

        // Hand the device build slot to an out-of-process supervisor now that
        // the direct build pid is known. The slot descriptor is close-on-exec,
        // so the build never inherits the lock; this process (`sharecli start`)
        // exits immediately after spawning, and without a supervisor the slot
        // would be released here instead of at the build's exit. Like the
        // registry write below, a failure must not fail the spawn: the child is
        // already running, and killing it would be worse than a slot released
        // at spawn-return. It is reported, never swallowed.
        if let Some(lease) = lease.as_ref() {
            if let Err(error) = lease.watch_until_exit(pid as i32) {
                eprintln!(
                    "sharecli: warning: could not hand build slot to a supervisor for pid \
                     {pid} ({error}); max_concurrent_builds is now enforced for this \
                     invocation's lifetime only"
                );
            }
        }

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

        // The slot travels with the entry: dropping it here would release the
        // build slot at spawn-return instead of at child exit.
        let managed = ManagedProcess { info: info.clone(), handle, lease, permit };

        let mut procs = self.processes.write().await;
        procs.insert(pid, managed);
        drop(procs);

        if let Err(error) = self.registry().insert(RegistryEntry::new(
            pid,
            info.project.clone(),
            info.harness.clone(),
            info.cmd.clone(),
        )) {
            // A registry write failure must not fail the spawn: the child is
            // already running and killing it here would be worse than a pid the
            // next invocation cannot see.
            eprintln!(
                "sharecli: warning: could not record pid {pid} for later invocations: {error}"
            );
        }

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
        self.registry().remove(pid);
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
            self.registry().remove(pid);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(failures.join("; ")))
        }
    }

    /// Kill a process recorded by an earlier `sharecli` invocation.
    ///
    /// When this invocation owns the handle the substrate group-kill is used;
    /// otherwise the device registry supplies the pid and the signal goes to its
    /// process group, so a harness's children exit with it.
    pub async fn kill_registered(&self, pid: u32, force: bool) -> Result<()> {
        // Scope the read guard: `kill` takes the write lock, and tokio's RwLock is
        // not reentrant, so holding it across the call would deadlock.
        let owns_handle = { self.processes.read().await.contains_key(&pid) };
        if owns_handle {
            return self.kill(pid).await;
        }

        let registry = self.registry();
        let Some(entry) = registry.get(pid) else {
            return Err(anyhow!("pid {pid} is not a sharecli-managed process"));
        };

        // The pid may have exited since it was recorded: on unix the group
        // signal would have reached any survivors, so a gone leader is success.
        // On Windows nothing was ever signalled and a dead pid cannot identify
        // surviving descendants, so `Ok` here would repeat the silent
        // partial-kill fault from the other side of the race.
        if !pid_alive(pid) {
            registry.remove(pid);
            #[cfg(windows)]
            return Err(anyhow!(
                "pid {pid} already exited; surviving descendants (if any) cannot be \
                 identified or stopped on Windows without a spawn-time Job Object"
            ));
            #[cfg(not(windows))]
            return Ok(());
        }
        // A recycled PID must never be signalled. `kill(pid, 0)` succeeds for
        // whatever now owns the id, so validate the recorded identity first and
        // drop the record instead of signalling an unrelated process.
        if !identity_matches_live(&entry) {
            registry.remove(pid);
            return Err(anyhow!(
                "pid {pid} is no longer the recorded process (stale or recycled pid); \
                 removed the stale registry entry instead of signalling it"
            ));
        }

        #[cfg(unix)]
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        // Windows has no POSIX signals. `signal_group_or_pid` refuses there
        // because a bare pid cannot prove tree ownership, so this `stop` fails
        // closed instead of performing a single-pid kill that reports success.
        #[cfg(not(unix))]
        let signal = if force { 9 } else { 15 };
        let label = if force { "SIGKILL" } else { "SIGTERM" };
        if let Err(error) = signal_group_or_pid(pid, signal) {
            // The target may have exited between the identity probe and the
            // signal. On unix that is success and the stale entry must go; on
            // Windows the refusal was never a signal, so the exit does not make
            // the tree state known: keep the error and drop the dead record.
            if !pid_alive(pid) {
                registry.remove(pid);
                #[cfg(not(windows))]
                return Ok(());
            }
            return Err(anyhow!("pid {pid}: {error}"));
        }

        // Confirm the exit for BOTH signal strengths. A registry entry is
        // removed iff the process is observed gone, so `Ok` never accompanies a
        // live pid, and a failed SIGKILL can no longer look like success.
        for _ in 0..40 {
            if !pid_alive(pid) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        if pid_alive(pid) {
            let hint = if force { "" } else { "; retry with --force" };
            return Err(anyhow!("pid {pid} did not exit after {label}{hint}"));
        }
        registry.remove(pid);
        Ok(())
    }

    /// Kill every process recorded on this device, including ones started by
    /// earlier invocations.
    pub async fn kill_all_registered(&self, force: bool) -> Result<()> {
        let mut failures = Vec::new();
        // Owned handles first: a child spawned by this invocation whose registry
        // write failed is in `processes` but not in the registry, and a
        // device-wide stop must not leave it running.
        if let Err(error) = self.kill_all().await {
            failures.push(format!("owned handles: {error}"));
        }
        // Snapshot the recorded set *before* pruning. `prune_dead` returns only
        // the entries it kept live, so a record whose leader already exited is
        // dropped and never handed to `kill_registered`. Without this snapshot a
        // device-wide stop would silently succeed for that record, even though
        // `stop --pid` refuses to call the same state a success. Report every
        // pruned record as an explicit failure so the outcome is identical on
        // both platforms.
        let recorded_before = self.registry().entries();
        match self.registry().prune_dead() {
            Ok(entries) => {
                let stopped: std::collections::HashSet<u32> =
                    entries.iter().map(|entry| entry.pid).collect();
                for entry in &recorded_before {
                    if !stopped.contains(&entry.pid) {
                        // Another writer may have removed an entry between the
                        // snapshot and the prune. That reports one conservative
                        // extra failure, never a false success, and this call
                        // never signalled that record either way.
                        failures.push(format!(
                            "pid {}: already exited and was pruned from the registry; \
                             the process tree was not signalled",
                            entry.pid
                        ));
                    }
                }
                for entry in entries {
                    if let Err(error) = self.kill_registered(entry.pid, force).await {
                        failures.push(format!("pid {}: {error}", entry.pid));
                    }
                }
            }
            Err(error) => {
                // An unreadable registry looks like "no live processes" if it
                // is swallowed. Surface it so a device-wide stop can never
                // report success while recorded processes keep running.
                failures.push(format!("registry: {error}"));
            }
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
        // Registry resolution must happen before any async guard is taken:
        // the registry read is synchronous file I/O. `find` is a read path, so
        // it must not rewrite the registry.
        //
        // Read the *unvalidated* snapshot here and validate identity only after
        // `refresh()` below. Validating first left a window in which a PID
        // recycled before the sysinfo snapshot was rendered as the replacement
        // process; the identity probe must be at least as fresh as the snapshot
        // it is qualifying.
        let recorded = self.registry().entries();
        self.refresh().await;
        let sys = self.system.read().await;

        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        {
            let procs = self.processes.read().await;
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
                        seen.insert(info.pid);
                        result.push(info);
                    }
                }
            }
        }

        for entry in recorded {
            if seen.contains(&entry.pid) {
                continue;
            }
            // Re-probe the recorded identity now that the sysinfo snapshot is
            // current. A pid recycled between the registry read and the refresh
            // fails here and is never rendered as the recorded process.
            if !entry_is_live(&entry) {
                continue;
            }
            let matches = match &filter {
                ProcessFilter::All => true,
                ProcessFilter::ByProject(proj) => entry.project.as_ref() == Some(proj),
                ProcessFilter::ByHarness(harness) => entry.harness.as_ref() == Some(harness),
            };
            if !matches {
                continue;
            }
            let name = entry.cmd.first().cloned().unwrap_or_default();
            if let Some(mut info) = ProcessInfo::from_sysinfo(Pid::from_u32(entry.pid), name, &sys)
            {
                info.project = entry.project.clone();
                info.harness = entry.harness.clone();
                result.push(info);
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

#[cfg(test)]
mod stop_all_regression_tests {
    use super::*;

    /// A private registry file, mirroring `runtime::tests` so this module never
    /// touches the device-wide registry.
    fn isolated_registry_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sharecli-pool-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create isolated registry dir");
        dir.join("processes.json")
    }

    /// Review `PRRT_kwDORwQ5fs67Oa1H`: a recorded leader that already exited is
    /// pruned by `prune_dead` and never handed to `kill_registered`, so a
    /// device-wide stop used to report success without ever having signalled it.
    /// It must now be surfaced as an explicit failure on both platforms.
    #[tokio::test]
    async fn kill_all_registered_reports_pruned_dead_record_as_failure() {
        let path = isolated_registry_path("stop-all-dead-record");
        let pool = ProcessPool::with_registry_path(path.clone());
        let registry = Registry::at(path);
        // PID 0 is never signalable, so it is dead without spawning anything.
        registry
            .insert(RegistryEntry::new(0, None, None, vec!["dead".into()]))
            .expect("seed dead record");

        let error = pool
            .kill_all_registered(false)
            .await
            .expect_err("a pruned dead record must not be silent success");

        assert!(error.to_string().contains("already exited"), "got: {error}");
        assert!(!registry.contains(0), "the dead record must still be pruned from the file");
    }
}
