//! Process runtime management with shared pool support.
//!
//! Spawn/monitor/kill delegated to substrate [`ProcessPort`] (`runtime-process`);
//! sharecli retains metadata tagging, pooling, and resource limits.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use runtime_process::CommandGroupProcess;
use serde_json::json;
use substrate::{ProcessHandle, ProcessPort, ProcessSpawnSpec};
use sysinfo::{Pid, ProcessStatus, System};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::audit_log;
use crate::config;
use crate::spawn_policy::{is_build_harness, SpawnPolicy};

fn spawn_capability(cmd: &str, harness: &Option<String>) -> String {
    harness.clone().unwrap_or_else(|| cmd.to_string())
}

fn emit_spawn_audit(project: &Option<String>, capability: &str, outcome: &str, pid: Option<u32>) {
    let mut fields = json!({
        "project": project,
        "capability": capability,
        "outcome": outcome,
    });
    if let Some(pid) = pid {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("pid".to_string(), json!(pid));
        }
    }
    audit_log::emit_if_configured("spawn", fields);
}

fn emit_stop_audit(project: &Option<String>, capability: &str, pid: u32, outcome: &str) {
    audit_log::emit_if_configured(
        "stop",
        json!({
            "project": project,
            "pid": pid,
            "capability": capability,
            "outcome": outcome,
        }),
    );
}

/// Lightweight `ProcessStatus` value that survives serialisation. sysinfo's enum
/// has many variants gated per-platform — for IPC purposes we only need the
/// handful users ever see on the dashboard, mapped through this enum so the
/// wire format is stable across the Linux/macOS/Windows tray crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcState {
    Idle,
    Run,
    Sleep,
    Stop,
    Zombie,
    Tracing,
    Dead,
    #[default]
    Unknown,
}

impl From<ProcessStatus> for ProcState {
    fn from(s: ProcessStatus) -> Self {
        match s {
            ProcessStatus::Idle => Self::Idle,
            ProcessStatus::Run => Self::Run,
            ProcessStatus::Sleep => Self::Sleep,
            ProcessStatus::Stop => Self::Stop,
            ProcessStatus::Zombie => Self::Zombie,
            ProcessStatus::Tracing => Self::Tracing,
            ProcessStatus::Dead => Self::Dead,
            // Wakekill / Waking / Parked / Blocked / Unknown collapse to Unknown
            // on the dashboard. The exact variant is rarely useful to the user.
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
// The bin crate only writes most of these fields; they are consumed by the
// tray dashboard consumers (IPC payloads / JSON snapshots) outside this
// crate's compile unit, which the dead-code pass cannot see.
#[allow(dead_code)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    pub start_time: u64,
    /// CPU utilization percentage reported by `sysinfo` (0..100 * num_cores).
    /// Requires sysinfo to have collected at least two samples; the first
    /// refresh after a process start reports 0.
    pub cpu_percent: f32,
    pub project: Option<String>,
    pub harness: Option<String>,
    /// Parent PID (sysinfo extension trait). `None` for kernel threads / early
    /// boot processes. Used by the tray dashboard tree view.
    pub ppid: Option<u32>,
    /// Current working directory (sysinfo extension trait). Empty when the
    /// platform doesn't expose it (unknown/apple-sandbox).
    pub cwd: Option<String>,
    /// Number of environment variables. Computed from `sysinfo::Process::environ()`
    /// length; cross-platform.
    pub env_count: u32,
    /// Process state (Idle/Run/Sleep/etc.) mapped through `ProcState` for
    /// stable serialisation. Used by tray dashboard state-color coding.
    pub state: ProcState,
    /// Total bytes read from disk (Linux-only via `disk_usage()`). `None` on
    /// macOS, Windows, and other platforms. Used by tray dashboard Io column.
    pub disk_read_bytes: Option<u64>,
    /// Total bytes written to disk (Linux-only). `None` on non-Linux.
    pub disk_write_bytes: Option<u64>,
    /// Number of open file descriptors. Computed via `lsof -p <pid>` on
    /// all platforms (cross-platform, ~20ms per process). `None` if the
    /// process is not accessible or `lsof` is unavailable.
    pub fd_count: Option<u32>,
    /// Thread count. Computed via `lsof -p <pid> -F f | grep '^t' | wc -l`
    /// on all platforms. `None` if inaccessible.
    pub thread_count: Option<u32>,
}

impl ProcessInfo {
    pub fn from_sysinfo(pid: Pid, name: String, sys: &System) -> Option<Self> {
        let p = sys.process(pid)?;
        // sysinfo 0.39: parent/cwd/environ/status/disk_usage are direct
        // methods on Process (no ProcessExt trait, which was removed in 0.32).
        #[cfg(unix)]
        let ppid = p.parent().map(|p| p.as_u32());
        #[cfg(not(unix))]
        let ppid: Option<u32> = None;

        #[cfg(unix)]
        let cwd = {
            let s = p.cwd().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        };
        #[cfg(not(unix))]
        let cwd: Option<String> = None;

        #[cfg(unix)]
        let env_count = p.environ().len() as u32;
        #[cfg(not(unix))]
        let env_count: u32 = 0;

        let state: ProcState = {
            #[cfg(unix)]
            {
                p.status().into()
            }
            #[cfg(not(unix))]
            ProcState::Unknown
        };

        #[cfg(target_os = "linux")]
        let (disk_read_bytes, disk_write_bytes) = {
            let du = p.disk_usage();
            (Some(du.total_read_bytes), Some(du.total_written_bytes))
        };
        let fd_count = count_open_fds(pid.as_u32());
        let thread_count = count_threads(pid.as_u32());

        #[cfg(not(target_os = "linux"))]
        let (disk_read_bytes, disk_write_bytes): (Option<u64>, Option<u64>) = (None, None);

        Some(ProcessInfo {
            pid: pid.as_u32(),
            name,
            cmd: p.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect(),
            memory_mb: p.memory() / 1024 / 1024,
            start_time: p.start_time(),
            cpu_percent: p.cpu_usage(),
            project: None,
            harness: None,
            ppid,
            cwd,
            env_count,
            state,
            fd_count,
            thread_count,
            disk_read_bytes,
            disk_write_bytes,
        })
    }
}

/// Count descriptors without crossing the runtime/IPC layer boundary.
/// Linux uses `/proc` first; macOS and other Unix systems fall back to lsof.
fn count_open_fds(pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    if let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) {
        return Some(entries.filter_map(std::result::Result::ok).count() as u32);
    }

    for path in ["/usr/sbin/lsof", "/usr/bin/lsof", "/bin/lsof"] {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let output = std::process::Command::new(path)
            .args(["-p", &pid.to_string(), "-F", "f"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        return Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.starts_with('f') && line.len() > 1)
                .count() as u32,
        );
    }
    None
}

fn count_threads(pid: u32) -> Option<u32> {
    #[allow(clippy::needless_return)]
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/task")) {
            return Some(entries.filter_map(std::result::Result::ok).count() as u32);
        }
        None
    }

    #[cfg(target_os = "macos")]
    #[allow(clippy::needless_return)]
    {
        let output = std::process::Command::new("/bin/ps")
            .args(["-M", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        return Some(String::from_utf8_lossy(&output.stdout).lines().skip(1).count() as u32);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

// ---------------------------------------------------------------------------
// RAII env-var guard — restores a variable to its previous value on drop.
// Used to temporarily inject CARGO_BUILD_JOBS / RUSTC_WRAPPER for a spawn.
// ---------------------------------------------------------------------------

struct EnvGuard {
    key: String,
    prev: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var(&self.key, v) },
            None => unsafe { std::env::remove_var(&self.key) },
        }
    }
}

#[derive(Debug)]
pub struct ManagedProcess {
    pub info: ProcessInfo,
    pub handle: ProcessHandle,
}

pub struct ProcessPool {
    processes: RwLock<HashMap<u32, ManagedProcess>>,
    system: RwLock<System>,
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
            system: RwLock::new(System::new_all()),
            port: CommandGroupProcess::new(),
            spawn_policy: None,
        }
    }

    /// Create a `ProcessPool` with a build-contention throttle applied to
    /// cargo/rustc and other build harnesses.
    pub fn with_spawn_policy(policy: Arc<SpawnPolicy>) -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
            system: RwLock::new(System::new_all()),
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
                return Err(anyhow::anyhow!("spawn {spawn_program}: {e}"));
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
                    return Err(anyhow::anyhow!("kill pid {pid}: {e}"));
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
            Err(anyhow::anyhow!(failures.join("; ")))
        }
    }

    /// Get system memory usage
    pub async fn system_memory_usage(&self) -> (u64, u64) {
        let sys = self.system.read().await;
        (sys.used_memory() / 1024 / 1024, sys.total_memory() / 1024 / 1024)
    }
}

/// Shared runtime pool for node/bun processes
pub struct SharedRuntime {
    node_pool: RwLock<Vec<PooledProcess>>,
    bun_pool: RwLock<Vec<PooledProcess>>,
    max_per_type: usize,
    system: Arc<RwLock<System>>,
}

#[derive(Debug, Clone)]
pub struct PooledProcess {
    pub pid: u32,
    pub name: String,
    pub in_use: bool,
    pub last_used: Instant,
}

impl SharedRuntime {
    pub fn new(max_per_type: usize) -> Self {
        Self {
            node_pool: RwLock::new(Vec::new()),
            bun_pool: RwLock::new(Vec::new()),
            max_per_type,
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }

    pub async fn refresh(&self) {
        let mut sys = self.system.write().await;
        sys.refresh_all();
    }

    pub async fn acquire(&self, harness_type: &str) -> Result<PooledProcess> {
        let pool = match harness_type {
            "node" => &self.node_pool,
            "bun" => &self.bun_pool,
            _ => bail!("Unsupported harness type: {}. Use 'node' or 'bun'", harness_type),
        };

        let mut pool_guard = pool.write().await;

        if let Some(idx) = pool_guard.iter().position(|p| !p.in_use) {
            pool_guard[idx].in_use = true;
            pool_guard[idx].last_used = Instant::now();
            return Ok(pool_guard[idx].clone());
        }

        if pool_guard.len() < self.max_per_type {
            let (cmd, name) = match harness_type {
                "node" => ("node", "node"),
                "bun" => ("bun", "bun"),
                _ => unreachable!(),
            };

            let child = Command::new(cmd)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;

            let pid = child.id().unwrap_or(0);
            drop(child);

            tokio::time::sleep(Duration::from_millis(config::global().pool.spawn_delay_ms)).await;
            self.refresh().await;

            let sys = self.system.read().await;
            let pooled = if sys.process(Pid::from_u32(pid)).is_some() {
                PooledProcess {
                    pid,
                    name: name.to_string(),
                    in_use: true,
                    last_used: Instant::now(),
                }
            } else {
                bail!(
                    "Failed to spawn pooled process — check that '{}' is installed and on PATH",
                    harness_type
                );
            };

            pool_guard.push(pooled.clone());
            Ok(pooled)
        } else {
            bail!(
                "Pool exhausted: max {} instances of {} allowed. Set pool.max_per_type in config.toml to increase the limit.",
                self.max_per_type,
                harness_type
            );
        }
    }

    pub async fn release(&self, harness_type: &str, pid: u32) -> Result<()> {
        let pool = match harness_type {
            "node" => &self.node_pool,
            "bun" => &self.bun_pool,
            _ => bail!("Unsupported harness type: {}", harness_type),
        };

        let mut pool_guard = pool.write().await;
        if let Some(p) = pool_guard.iter_mut().find(|p| p.pid == pid) {
            p.in_use = false;
            p.last_used = Instant::now();
        }
        Ok(())
    }

    pub async fn run_with_pool(
        &self,
        harness_type: &str,
        project: &str,
        _script: &str,
    ) -> Result<(u32, String)> {
        let pooled = self.acquire(harness_type).await?;

        let output =
            format!("Using pooled {} process {} for project {}", harness_type, pooled.pid, project);

        self.release(harness_type, pooled.pid).await?;

        Ok((pooled.pid, output))
    }

    pub async fn health_check(&self) -> RuntimeHealth {
        self.refresh().await;

        let sys = self.system.read().await;
        let node_pool = self.node_pool.read().await;
        let bun_pool = self.bun_pool.read().await;

        let mut healthy = true;
        let mut issues = Vec::new();

        for p in node_pool.iter().chain(bun_pool.iter()) {
            if let Some(proc) = sys.process(Pid::from_u32(p.pid)) {
                if proc.memory() > config::global().monitoring.per_process_warn_memory_bytes {
                    issues.push(format!(
                        "{} (PID {}) using {} MB - high memory",
                        p.name,
                        p.pid,
                        proc.memory() / 1024 / 1024
                    ));
                }
            } else {
                healthy = false;
                issues.push(format!("{} (PID {}) not found - may have crashed", p.name, p.pid));
            }
        }

        RuntimeHealth {
            healthy,
            issues,
            node_in_use: node_pool.iter().filter(|p| p.in_use).count(),
            bun_in_use: bun_pool.iter().filter(|p| p.in_use).count(),
        }
    }

    pub async fn status(&self) -> PoolStatus {
        let node_pool = self.node_pool.read().await;
        let bun_pool = self.bun_pool.read().await;

        PoolStatus {
            node_total: node_pool.len(),
            node_idle: node_pool.iter().filter(|p| !p.in_use).count(),
            bun_total: bun_pool.len(),
            bun_idle: bun_pool.iter().filter(|p| !p.in_use).count(),
            max_per_type: self.max_per_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeHealth {
    pub healthy: bool,
    pub issues: Vec<String>,
    pub node_in_use: usize,
    pub bun_in_use: usize,
}

#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub node_total: usize,
    pub node_idle: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub max_per_type: usize,
}

/// Project resource limits
#[derive(Debug, Clone)]
pub struct ProjectLimits {
    pub memory_limit_mb: u64,
    pub max_processes: usize,
    pub cpu_affinity: Option<Vec<usize>>,
}

impl Default for ProjectLimits {
    fn default() -> Self {
        let cfg = config::global();
        Self {
            memory_limit_mb: cfg.project_limits.memory_limit_mb,
            max_processes: cfg.project_limits.max_processes,
            cpu_affinity: None,
        }
    }
}

/// Project resource manager
pub struct ProjectResources {
    projects: RwLock<HashMap<String, ProjectLimits>>,
    system: Arc<RwLock<System>>,
}

impl Default for ProjectResources {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectResources {
    pub fn new() -> Self {
        Self {
            projects: RwLock::new(HashMap::new()),
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }

    pub async fn set_limits(&self, name: &str, limits: ProjectLimits) {
        let mut projects = self.projects.write().await;
        projects.insert(name.to_string(), limits);
    }

    pub async fn get_limits(&self, name: &str) -> ProjectLimits {
        let projects = self.projects.read().await;
        projects.get(name).cloned().unwrap_or_default()
    }

    pub async fn check_limits(&self, project: &str) -> Result<ResourceCheck> {
        self.refresh();
        let sys = self.system.read().await;
        let limits = self.get_limits(project).await;

        let mut total_memory = 0u64;
        let mut process_count = 0usize;

        for proc in sys.processes().values() {
            let cmd: Vec<String> =
                proc.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect();
            if cmd.iter().any(|c| c.contains(project)) {
                total_memory += proc.memory() / 1024 / 1024;
                process_count += 1;
            }
        }

        let memory_ok = total_memory <= limits.memory_limit_mb;
        let processes_ok = process_count <= limits.max_processes;

        Ok(ResourceCheck {
            memory_mb: total_memory,
            memory_limit_mb: limits.memory_limit_mb,
            memory_ok,
            process_count,
            max_processes: limits.max_processes,
            processes_ok,
            overall_ok: memory_ok && processes_ok,
        })
    }

    fn refresh(&self) {
        let sys = self.system.clone();
        tokio::spawn(async move {
            let mut s = sys.write().await;
            s.refresh_all();
        });
    }
}

#[derive(Debug, Clone)]
pub struct ResourceCheck {
    pub memory_mb: u64,
    pub memory_limit_mb: u64,
    pub memory_ok: bool,
    pub process_count: usize,
    pub max_processes: usize,
    pub processes_ok: bool,
    pub overall_ok: bool,
}

/// Filter for specific process types
#[derive(Debug, Clone)]
pub enum ProcessFilter {
    All,
    ByProject(String),
    ByHarness(String),
}

impl ProcessPool {
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
                let matches = match filter {
                    ProcessFilter::All => true,
                    ProcessFilter::ByProject(ref proj) => info.project.as_ref() == Some(proj),
                    ProcessFilter::ByHarness(ref harness) => info.harness.as_ref() == Some(harness),
                };

                if matches {
                    result.push(info);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_unowned_handle(pool: &ProcessPool) -> u32 {
        let pid = std::process::id();
        let system = pool.system.read().await;
        let info = ProcessInfo::from_sysinfo(Pid::from_u32(pid), "fixture".into(), &system)
            .expect("test process must be visible");
        drop(system);
        pool.processes.write().await.insert(
            pid,
            ManagedProcess { info, handle: ProcessHandle { id: Default::default(), pid } },
        );
        pid
    }

    #[tokio::test]
    async fn failed_kill_retains_tracking() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        assert!(pool.kill(pid).await.is_err());
        assert!(pool.processes.read().await.contains_key(&pid));
    }

    #[tokio::test]
    async fn failed_kill_all_reports_error_and_retains_tracking() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        let result = pool.kill_all().await;
        assert!(result.is_err(), "failed stop must not acknowledge success");
        assert!(pool.processes.read().await.contains_key(&pid));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_all_removes_successful_children_but_retains_failures() {
        let pool = ProcessPool::new();
        let child = pool.spawn("sleep", &["30".into()], None, None, None).await.unwrap();
        let failed_pid = insert_unowned_handle(&pool).await;
        assert!(pool.kill_all().await.is_err());
        let tracked = pool.processes.read().await;
        assert!(tracked.contains_key(&failed_pid));
        assert!(!tracked.contains_key(&child.pid));
        assert_eq!(tracked.len(), 1);
    }

    #[tokio::test]
    async fn test_process_pool() {
        let pool = ProcessPool::new();

        #[cfg(unix)]
        let (cmd, args) = ("sleep", vec!["1".to_string()]);
        #[cfg(windows)]
        let (cmd, args) = (
            "cmd",
            vec![
                "/C".to_string(),
                "ping".to_string(),
                "127.0.0.1".to_string(),
                "-n".to_string(),
                "2".to_string(),
            ],
        );

        let info = pool.spawn(cmd, &args, None, None, None).await;
        assert!(info.is_ok(), "process-pool spawn failed: {info:?}");

        let list = pool.list().await;
        assert!(!list.is_empty());

        pool.kill_all().await.unwrap();
    }

    // -----------------------------------------------------------------------
    // spawn_capability
    // -----------------------------------------------------------------------

    #[test]
    fn spawn_capability_uses_harness_when_provided() {
        assert_eq!(spawn_capability("cargo", &Some("rustc".into())), "rustc");
    }

    #[test]
    fn spawn_capability_falls_back_to_cmd_name() {
        assert_eq!(spawn_capability("cargo", &None), "cargo");
    }

    #[test]
    fn spawn_capability_empty_harness_returns_empty_string() {
        // Some("") is Some, so unwrap_or_else does NOT trigger the fallback.
        assert_eq!(spawn_capability("node", &Some(String::new())), "");
    }

    // -----------------------------------------------------------------------
    // ProcState conversions
    // -----------------------------------------------------------------------

    #[test]
    fn proc_state_default_is_unknown() {
        let s = ProcState::default();
        assert_eq!(s, ProcState::Unknown);
    }

    #[test]
    fn proc_state_from_sysinfo_variants() {
        // Map every sysinfo ProcessStatus to a ProcState and check known ones.
        use sysinfo::ProcessStatus;
        let pairs = vec![
            (ProcessStatus::Idle, ProcState::Idle),
            (ProcessStatus::Run, ProcState::Run),
            (ProcessStatus::Sleep, ProcState::Sleep),
            (ProcessStatus::Stop, ProcState::Stop),
            (ProcessStatus::Zombie, ProcState::Zombie),
            (ProcessStatus::Tracing, ProcState::Tracing),
            (ProcessStatus::Dead, ProcState::Dead),
        ];
        for (sys, expected) in pairs {
            let state: ProcState = sys.into();
            assert_eq!(state, expected, "mapping for {:?}", sys);
        }
    }

    #[test]
    fn proc_state_is_serializable_round_trip() {
        let states = vec![
            ProcState::Idle,
            ProcState::Run,
            ProcState::Sleep,
            ProcState::Stop,
            ProcState::Zombie,
            ProcState::Tracing,
            ProcState::Dead,
            ProcState::Unknown,
        ];
        for s in states {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: ProcState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, s, "round-trip for {:?}", s);
        }
    }

    #[test]
    fn proc_state_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&ProcState::Run).unwrap(), "\"run\"");
        assert_eq!(serde_json::to_string(&ProcState::Sleep).unwrap(), "\"sleep\"");
        assert_eq!(serde_json::to_string(&ProcState::Zombie).unwrap(), "\"zombie\"");
    }

    // -----------------------------------------------------------------------
    // SharedRuntime
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn shared_runtime_status_initially_empty() {
        let rt = SharedRuntime::new(4);
        let status = rt.status().await;
        assert_eq!(status.node_total, 0);
        assert_eq!(status.node_idle, 0);
        assert_eq!(status.bun_total, 0);
        assert_eq!(status.bun_idle, 0);
        assert_eq!(status.max_per_type, 4);
    }

    #[tokio::test]
    async fn shared_runtime_acquire_unsupported_type_fails() {
        let rt = SharedRuntime::new(4);
        let result = rt.acquire("python").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported harness type"), "got: {err}");
    }

    #[tokio::test]
    async fn shared_runtime_release_unsupported_type_fails() {
        let rt = SharedRuntime::new(4);
        let result = rt.release("python", 123).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported harness type"), "got: {err}");
    }

    #[tokio::test]
    async fn shared_runtime_release_nonexistent_pid_is_ok() {
        let rt = SharedRuntime::new(4);
        // Releasing a PID that doesn't exist should succeed silently.
        assert!(rt.release("node", 99999).await.is_ok());
    }

    #[tokio::test]
    async fn shared_runtime_health_check_empty_pools() {
        let rt = SharedRuntime::new(4);
        let health = rt.health_check().await;
        assert!(health.healthy);
        assert!(health.issues.is_empty());
        assert_eq!(health.node_in_use, 0);
        assert_eq!(health.bun_in_use, 0);
    }

    // -----------------------------------------------------------------------
    // ProjectResources
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn project_resources_get_default_when_unset() {
        let _ = crate::config::init_global();
        let pr = ProjectResources::new();
        let limits = pr.get_limits("nonexistent").await;
        // Default should come from config::global().project_limits
        let cfg = crate::config::global();
        assert_eq!(limits.memory_limit_mb, cfg.project_limits.memory_limit_mb);
        assert_eq!(limits.max_processes, cfg.project_limits.max_processes);
        assert!(limits.cpu_affinity.is_none());
    }

    #[tokio::test]
    async fn project_resources_set_and_get() {
        let pr = ProjectResources::new();
        let custom = ProjectLimits {
            memory_limit_mb: 2048,
            max_processes: 8,
            cpu_affinity: Some(vec![0, 1]),
        };
        pr.set_limits("my-project", custom.clone()).await;
        let got = pr.get_limits("my-project").await;
        assert_eq!(got.memory_limit_mb, 2048);
        assert_eq!(got.max_processes, 8);
        assert_eq!(got.cpu_affinity, Some(vec![0, 1]));
    }

    #[tokio::test]
    async fn project_resources_check_limits_returns_ok() {
        let _ = crate::config::init_global();
        let pr = ProjectResources::new();
        // check_limits should succeed even for empty projects (0 processes <= max_processes)
        let check = pr.check_limits("nonexistent-project").await;
        assert!(check.is_ok());
        let rc = check.unwrap();
        assert!(rc.processes_ok);
        assert!(rc.overall_ok);
    }

    // -----------------------------------------------------------------------
    // ProjectLimits Default
    // -----------------------------------------------------------------------

    #[test]
    fn project_limits_default_uses_config() {
        let _ = crate::config::init_global();
        let limits = ProjectLimits::default();
        let cfg = crate::config::global();
        assert_eq!(limits.memory_limit_mb, cfg.project_limits.memory_limit_mb);
        assert_eq!(limits.max_processes, cfg.project_limits.max_processes);
    }

    // -----------------------------------------------------------------------
    // ProcessInfo construction helpers
    // -----------------------------------------------------------------------

    #[test]
    fn process_info_debug_clone() {
        let info = ProcessInfo {
            pid: 42,
            name: "test-proc".into(),
            cmd: vec!["test".into(), "--flag".into()],
            memory_mb: 128,
            start_time: 1000,
            cpu_percent: 50.5,
            project: Some("alpha".into()),
            harness: Some("cargo".into()),
            ppid: Some(1),
            cwd: Some("/tmp".into()),
            env_count: 3,
            state: ProcState::Run,
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: Some(10),
            thread_count: Some(4),
        };
        let cloned = info.clone();
        assert_eq!(cloned.pid, 42);
        assert_eq!(cloned.name, "test-proc");
        assert_eq!(cloned.cmd, vec!["test", "--flag"]);
        assert_eq!(cloned.memory_mb, 128);
        assert_eq!(cloned.cpu_percent, 50.5);
        assert_eq!(cloned.project, Some("alpha".into()));
        assert_eq!(cloned.harness, Some("cargo".into()));
        assert_eq!(cloned.ppid, Some(1));
        assert_eq!(cloned.cwd, Some("/tmp".into()));
        assert_eq!(cloned.env_count, 3);
        assert_eq!(cloned.state, ProcState::Run);
        assert_eq!(cloned.fd_count, Some(10));
        assert_eq!(cloned.thread_count, Some(4));
    }

    #[test]
    fn process_info_defaults_for_optional_fields() {
        let info = ProcessInfo {
            pid: 1,
            name: "minimal".into(),
            cmd: vec![],
            memory_mb: 0,
            start_time: 0,
            cpu_percent: 0.0,
            project: None,
            harness: None,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        };
        assert!(info.project.is_none());
        assert!(info.harness.is_none());
        assert!(info.ppid.is_none());
        assert!(info.cwd.is_none());
    }

    // -----------------------------------------------------------------------
    // ProcessPool new + default
    // -----------------------------------------------------------------------

    #[test]
    fn process_pool_new_is_default() {
        let a = ProcessPool::new();
        let b = ProcessPool::default();
        // Both should be constructible. Check list returns empty for fresh pools.
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let la = a.list().await;
            let lb = b.list().await;
            assert!(la.is_empty());
            assert!(lb.is_empty());
        });
    }

    // -----------------------------------------------------------------------
    // ProcessPool system_memory_usage
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn system_memory_usage_returns_nonzero() {
        let pool = ProcessPool::new();
        let (used, total) = pool.system_memory_usage().await;
        // A running system must have >0 total memory.
        assert!(total > 0, "total memory should be non-zero");
        assert!(used <= total, "used should be <= total");
    }

    // -----------------------------------------------------------------------
    // ProcessPool find filters
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn find_all_returns_managed_processes() {
        let pool = ProcessPool::new();
        let results = pool.find(ProcessFilter::All).await;
        // With no spawned processes, should be empty.
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn find_by_project_filters_correctly() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        {
            let mut procs = pool.processes.write().await;
            if let Some(m) = procs.get_mut(&pid) {
                m.info.project = Some("alpha".into());
            }
        }
        // Refresh so sysinfo can see the process.
        pool.refresh().await;
        let alpha = pool.find(ProcessFilter::ByProject("alpha".into())).await;
        assert!(!alpha.is_empty());
        assert!(alpha.iter().all(|p| p.project.as_deref() == Some("alpha")));

        let beta = pool.find(ProcessFilter::ByProject("beta".into())).await;
        assert!(beta.is_empty());
    }

    #[tokio::test]
    async fn find_by_harness_filters_correctly() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        {
            let mut procs = pool.processes.write().await;
            if let Some(m) = procs.get_mut(&pid) {
                m.info.harness = Some("cargo".into());
            }
        }
        pool.refresh().await;
        let cargo = pool.find(ProcessFilter::ByHarness("cargo".into())).await;
        assert!(!cargo.is_empty());
        assert!(cargo.iter().all(|p| p.harness.as_deref() == Some("cargo")));

        let node = pool.find(ProcessFilter::ByHarness("node".into())).await;
        assert!(node.is_empty());
    }

    // -----------------------------------------------------------------------
    // PooledProcess defaults
    // -----------------------------------------------------------------------

    #[test]
    fn pooled_process_clone() {
        let pp = PooledProcess {
            pid: 100,
            name: "node".into(),
            in_use: true,
            last_used: Instant::now(),
        };
        let cloned = pp.clone();
        assert_eq!(cloned.pid, 100);
        assert_eq!(cloned.name, "node");
        assert!(cloned.in_use);
    }

    // -----------------------------------------------------------------------
    // RuntimeHealth + PoolStatus + ResourceCheck
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_health_clone() {
        let h = RuntimeHealth {
            healthy: false,
            issues: vec!["oops".into()],
            node_in_use: 2,
            bun_in_use: 1,
        };
        let cloned = h.clone();
        assert!(!cloned.healthy);
        assert_eq!(cloned.issues, vec!["oops"]);
        assert_eq!(cloned.node_in_use, 2);
        assert_eq!(cloned.bun_in_use, 1);
    }

    #[test]
    fn pool_status_clone() {
        let s =
            PoolStatus { node_total: 3, node_idle: 1, bun_total: 2, bun_idle: 0, max_per_type: 5 };
        let cloned = s.clone();
        assert_eq!(cloned.node_total, 3);
        assert_eq!(cloned.bun_idle, 0);
    }

    #[test]
    fn resource_check_clone() {
        let rc = ResourceCheck {
            memory_mb: 100,
            memory_limit_mb: 200,
            memory_ok: true,
            process_count: 5,
            max_processes: 10,
            processes_ok: true,
            overall_ok: true,
        };
        let cloned = rc.clone();
        assert!(cloned.overall_ok);
        assert_eq!(cloned.memory_mb, 100);
    }

    // -----------------------------------------------------------------------
    // EnvGuard drop
    // -----------------------------------------------------------------------

    #[test]
    fn env_guard_restores_previous_value() {
        unsafe {
            let key = "_JCODE_TEST_ENV_GUARD_RESTORE_";
            std::env::remove_var(key);
            {
                let _guard = EnvGuard { key: key.to_string(), prev: None };
                std::env::set_var(key, "temporary");
                assert_eq!(std::env::var(key).unwrap(), "temporary");
            }
            // After drop, variable should be removed (prev was None).
            assert!(std::env::var(key).is_err());
        }
    }

    #[test]
    fn env_guard_restores_previous_value_when_some() {
        unsafe {
            let key = "_JCODE_TEST_ENV_GUARD_RESTORE_SOME_";
            std::env::set_var(key, "original");
            {
                let _guard = EnvGuard { key: key.to_string(), prev: Some("original".to_string()) };
                std::env::set_var(key, "temporary");
                assert_eq!(std::env::var(key).unwrap(), "temporary");
            }
            // After drop, should restore the original value.
            assert_eq!(std::env::var(key).unwrap(), "original");
            std::env::remove_var(key);
        }
    }

    // -----------------------------------------------------------------------
    // ProcessFilter debug
    // -----------------------------------------------------------------------

    #[test]
    fn process_filter_debug_impl() {
        let all = ProcessFilter::All;
        let proj = ProcessFilter::ByProject("x".into());
        let harness = ProcessFilter::ByHarness("y".into());
        assert_eq!(format!("{:?}", all), "All");
        assert_eq!(format!("{:?}", proj), "ByProject(\"x\")");
        assert_eq!(format!("{:?}", harness), "ByHarness(\"y\")");
    }
}
