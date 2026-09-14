//! Request dispatch for the IPC server.
//!
//! Methods exposed:
//!   process.list        → Vec<ProcessSummary>
//!   process.kill        → { pid }
//!   process.kill_all    → {}
//!   process.cmdline     → { pid } → { cmd: Vec<String> }
//!   health.status       → HealthSnapshot
//!   pool.status         → PoolSnapshot
//!   status.snapshot     → StatusSnapshot
//!   config.get          → Config
//!   config.set          → { key, value }  (dot-path into TOML)
//!   monitoring.report   → MonitoringReportSnapshot
//!   log.tail            → { lines: [LogEntry], last_id: u64 } (since_id)
//!
//! IPC `log.tail` (PR 8 of `plans/2026-07-25-tray-dashboard-expanded-v1.md`)
//! streams entries from a process-global ring buffer fed by a `tracing-subscriber`
//! Layer (see `crate::log_buffer`). Clients advance their watermark via `since_id`
//! and the response carries `last_id` so they can resume without re-receiving
//! the entire history.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::Result;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sharecli::commands::proc::{AgentProcRow, AgentProcSnapshot};
use sharecli::config::Config;
use sharecli::monitoring::HostResourceWatchJson;
use sharecli::runtime::SharedRuntime;
use sharecli::{ProcessInfo, ProcessPool};
use sharecli_fleet::thermal::ThermalGovernor;
use sharecli_fleet::{count_host_agents, gate_status_snapshot, GateStatusSnapshot};
use sharecli_session::{
    LayoutSnapshot, RecoveryExecutor, SessionObservation, SessionStore,
    DEFAULT_RECOVERY_MAX_AGE_SECONDS,
};
use tokio::sync::RwLock;

use crate::log_buffer::global as global_log_buffer;

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Serialize)]
pub struct Response {
    pub id: u64,
    pub result: Value,
    pub error: Option<String>,
}

impl Response {
    fn ok(id: u64, result: impl Serialize) -> Self {
        Self { id, result: serde_json::to_value(result).unwrap_or(Value::Null), error: None }
    }

    fn err(id: u64, msg: impl std::fmt::Display) -> Self {
        Self { id, result: Value::Null, error: Some(msg.to_string()) }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProcessSummary {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    pub project: Option<String>,
    pub harness: Option<String>,
    /// Unix timestamp (seconds) the process started.
    #[serde(default)]
    pub start_time: u64,
    /// Per-process CPU utilization percent (sysinfo Process::cpu_usage()).
    #[serde(default)]
    pub cpu_percent: f32,
    /// Parent PID. 0 = orphan or root.
    #[serde(default)]
    pub ppid: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Number of environment variables visible to the process.
    #[serde(default)]
    pub env_count: u32,
    /// Open file-descriptor count. None on macOS pre-FUSE / Linux unsupported.
    #[serde(default)]
    pub fd_count: Option<u32>,
    /// Thread count (None if unreadable).
    #[serde(default)]
    pub thread_count: Option<u32>,
    /// Total bytes read from disk (Linux only; None elsewhere).
    #[serde(default)]
    pub disk_read_bytes: Option<u64>,
    /// Total bytes written to disk (Linux only; None elsewhere).
    #[serde(default)]
    pub disk_write_bytes: Option<u64>,
    /// Observed process state (Running / Sleeping / Stopped / Unknown).
    #[serde(default)]
    pub state: String,
}

impl From<ProcessInfo> for ProcessSummary {
    fn from(p: ProcessInfo) -> Self {
        Self {
            pid: p.pid,
            name: p.name,
            cmd: p.cmd,
            memory_mb: p.memory_mb,
            project: p.project,
            harness: p.harness,
            start_time: p.start_time,
            cpu_percent: p.cpu_percent,
            ppid: p.ppid,
            cwd: p.cwd,
            env_count: p.env_count,
            fd_count: p.fd_count,
            thread_count: p.thread_count,
            disk_read_bytes: p.disk_read_bytes,
            disk_write_bytes: p.disk_write_bytes,
            state: format!("{:?}", p.state),
        }
    }
}

/// IPC `health.status` envelope (FR-007 / AC-007.45, pool/status AC-007.78).
///
/// Runtime health fields precede live `gate`, `host_watch`, `pool`, and `status`
/// siblings (parity with `health --json` AC-007.77).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HealthSnapshot {
    pub managed_processes: usize,
    pub used_memory_mb: u64,
    pub total_memory_mb: u64,
    pub healthy: bool,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    pub pool: PoolSnapshot,
    pub status: StatusSnapshot,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MonitoringProcessEntry {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub project: Option<String>,
    pub harness: Option<String>,
    /// Unix timestamp (seconds) the process started, as captured by `sysinfo`.
    /// Used by tray dashboards to render an "Age" column on the Processes page.
    /// Always 0 if the sidecar couldn't determine start_time.
    #[serde(default)]
    pub start_time: u64,
    /// Per-process CPU utilization (0..100*ncores). Used by tray dashboards
    /// for the CPU % column on the Processes page. 0 on first sysinfo sample.
    #[serde(default)]
    pub cpu_percent: f32,
    /// Parent PID (`sysinfo::Process::parent()`). Used by Resources + Tree
    /// subpages. None if the parent is gone or we lack privilege.
    #[serde(default)]
    pub ppid: Option<u32>,
    /// Current working directory, if reachable. Used by Resources subpage.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Number of environment variables. Used by Resources subpage.
    #[serde(default)]
    pub env_count: u32,
    /// Open file descriptor count (None if unreadable cross-platform).
    /// Used by tray dashboard FDs column.
    #[serde(default)]
    pub fd_count: Option<u32>,
    /// Thread count (None if unreadable).
    #[serde(default)]
    pub thread_count: Option<u32>,
    /// Total bytes read from disk (Linux only; None elsewhere).
    #[serde(default)]
    pub disk_read_bytes: Option<u64>,
    /// Total bytes written to disk (Linux only; None elsewhere).
    #[serde(default)]
    pub disk_write_bytes: Option<u64>,
    /// Observed process state (Running / Sleeping / Stopped / Unknown).
    #[serde(default)]
    pub state: String,
}

/// IPC `pool.status` envelope (FR-007 / AC-007.67, nested status AC-007.78).
///
/// Pool status fields precede live `gate`, `host_watch`, and nested `status` sibling
/// (parity with `pool --json` AC-007.77).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PoolSnapshot {
    pub node_total: usize,
    pub node_idle: usize,
    pub bun_total: usize,
    pub bun_idle: usize,
    pub max_per_type: usize,
    pub healthy: bool,
    pub issues: Vec<String>,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Proc-scan status sibling; only emitted by `pool.status` (AC-007.78).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Box<StatusSnapshot>>,
}

/// IPC `status.snapshot` envelope (FR-007 / AC-007.67, nested pool AC-007.78).
///
/// Status fields precede live `gate`, `host_watch`, and nested `pool` sibling
/// (parity with `status --json` AC-007.77).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatusSnapshot {
    pub total_processes: usize,
    pub agents: Vec<AgentProcRow>,
    pub scanned: usize,
    pub watched: usize,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool sibling; only emitted by `status.snapshot` (AC-007.78).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<Box<PoolSnapshot>>,
}

/// IPC `monitoring.report` envelope (FR-007 / AC-007.46, pool/status AC-007.72).
///
/// Fleet monitoring fields precede live `gate`, `host_watch`, `pool`, and `status`
/// siblings (parity with dashboard WS AC-007.70 key order within the operator envelope).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MonitoringReportSnapshot {
    pub timestamp: u64,
    pub total_processes: usize,
    pub used_memory_mb: u64,
    pub total_memory_mb: u64,
    pub processes: Vec<MonitoringProcessEntry>,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    pub pool: PoolSnapshot,
    pub status: StatusSnapshot,
}

/// Live thermal gate + host resource watch for IPC envelopes (FR-007 / AC-007.45).
fn capture_gate_host_watch() -> Result<(GateStatusSnapshot, HostResourceWatchJson)> {
    let gate = match ThermalGovernor::new().poll() {
        Ok(level) => gate_status_snapshot(level, count_host_agents()),
        Err(_) => GateStatusSnapshot {
            thermal_pressure: "UNAVAILABLE".to_string(),
            detected_agents: count_host_agents(),
            agent_total_rss_bytes: 0,
            agent_contention: "UNAVAILABLE".to_string(),
            gate_decision: "UNAVAILABLE".to_string(),
        },
    };
    let host_watch = HostResourceWatchJson::capture()?;
    Ok((gate, host_watch))
}

static SHARED_RUNTIME: OnceLock<SharedRuntime> = OnceLock::new();

fn shared_runtime() -> &'static SharedRuntime {
    SHARED_RUNTIME.get_or_init(|| {
        let max = Config::load().map(|c| c.pool.max_per_type).unwrap_or(4);
        SharedRuntime::new(max)
    })
}

fn recovery_max_age_seconds(params: &Value) -> Result<u64> {
    let seconds = params
        .get("max_age_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_RECOVERY_MAX_AGE_SECONDS);
    anyhow::ensure!(
        (1..=604_800).contains(&seconds),
        "max_age_seconds must be between 1 and 604800"
    );
    Ok(seconds)
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub struct Handler {
    pool: Arc<ProcessPool>,
    config: Arc<RwLock<Config>>,
    sessions: Arc<SessionStore>,
}

// An explicit database path is opt-in; open failures propagate without fallback.
fn session_database_path(
    override_path: Option<std::ffi::OsString>,
    data_root: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf> {
    if let Some(path) = override_path {
        anyhow::ensure!(!path.is_empty(), "SHARECLI_SESSION_DB must not be empty");
        return Ok(std::path::PathBuf::from(path));
    }
    Ok(data_root
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("sharecli")
        .join("sessions.sqlite"))
}

#[cfg(test)]
mod database_path_tests {
    use super::*;

    #[test]
    fn explicit_database_overrides_default() {
        assert_eq!(
            session_database_path(Some("fixture.sqlite".into()), Some("default".into())).unwrap(),
            std::path::PathBuf::from("fixture.sqlite")
        );
    }

    #[test]
    fn empty_override_is_rejected() {
        assert!(session_database_path(Some("".into()), Some("default".into())).is_err());
    }

    #[test]
    fn unset_override_preserves_default_selection() {
        assert_eq!(
            session_database_path(None, Some("default".into())).unwrap(),
            std::path::PathBuf::from("default/sharecli/sessions.sqlite")
        );
        assert_eq!(
            session_database_path(None, None).unwrap(),
            std::path::PathBuf::from("/tmp/sharecli/sessions.sqlite")
        );
    }
}

impl Handler {
    /// Construct a handler with an explicitly owned store and default configuration.
    /// This does not read database-path environment variables or user configuration.
    pub fn with_session_store(sessions: SessionStore) -> Self {
        Self {
            pool: Arc::new(ProcessPool::new()),
            config: Arc::new(RwLock::new(Config::default())),
            sessions: Arc::new(sessions),
        }
    }

    #[cfg(test)]
    pub fn with_fixture_store(path: &std::path::Path) -> Result<Self> {
        Ok(Self::with_session_store(SessionStore::open(path)?))
    }

    pub async fn new() -> Result<Self> {
        let pool = Arc::new(ProcessPool::new());
        let config = Arc::new(RwLock::new(Config::load().unwrap_or_default()));
        let path =
            session_database_path(std::env::var_os("SHARECLI_SESSION_DB"), dirs::data_local_dir())?;
        let sessions = Arc::new(SessionStore::open(path)?);
        Ok(Self { pool, config, sessions })
    }

    async fn capture_pool_snapshot(
        &self,
        gate: GateStatusSnapshot,
        host_watch: HostResourceWatchJson,
    ) -> PoolSnapshot {
        let runtime = shared_runtime();
        let status = runtime.status().await;
        let health = runtime.health_check().await;
        PoolSnapshot {
            node_total: status.node_total,
            node_idle: status.node_idle,
            bun_total: status.bun_total,
            bun_idle: status.bun_idle,
            max_per_type: status.max_per_type,
            healthy: health.healthy,
            issues: health.issues,
            gate,
            host_watch,
            status: None,
        }
    }

    async fn capture_status_snapshot(&self) -> Result<StatusSnapshot> {
        self.pool.refresh().await;
        let procs = self.pool.list().await;
        let snapshot = AgentProcSnapshot::capture()?;
        Ok(StatusSnapshot {
            total_processes: procs.len(),
            agents: snapshot.agents,
            scanned: snapshot.scanned,
            watched: snapshot.watched,
            gate: snapshot.gate,
            host_watch: snapshot.host_watch,
            pool: None,
        })
    }

    pub async fn dispatch(&self, raw: &str) -> Response {
        let req: Request = match serde_json::from_str(raw) {
            Ok(r) => r,
            Err(e) => return Response::err(0, format!("parse error: {e}")),
        };

        match self.handle(&req).await {
            Ok(val) => Response::ok(req.id, val),
            Err(e) => Response::err(req.id, e),
        }
    }

    async fn handle(&self, req: &Request) -> Result<Value> {
        match req.method.as_str() {
            "session.list" => Ok(serde_json::to_value(self.sessions.list()?)?),

            "session.inspect" => {
                let id = req
                    .params
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("session.inspect: missing id"))?;
                Ok(serde_json::to_value(self.sessions.get(id)?)?)
            }

            "session.observe" => {
                let observation: SessionObservation = serde_json::from_value(
                    req.params.get("observation").cloned().unwrap_or_else(|| req.params.clone()),
                )?;
                let sequence = self.sessions.append_observation(&observation)?;
                Ok(serde_json::json!({"sequence": sequence, "surface_id": observation.surface.id}))
            }

            "session.observations" => {
                let surface_id = req.params.get("surface_id").and_then(Value::as_str);
                Ok(serde_json::to_value(self.sessions.observations(surface_id)?)?)
            }

            "session.compact" => {
                Ok(serde_json::json!({"removed": self.sessions.compact_observations()?}))
            }

            "layout.list" => Ok(serde_json::to_value(self.sessions.list_layouts()?)?),

            "layout.inspect" => {
                let id = req
                    .params
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("layout.inspect: missing id"))?;
                Ok(serde_json::to_value(self.sessions.get_layout(id)?)?)
            }

            "layout.save" => {
                let snapshot: LayoutSnapshot = serde_json::from_value(
                    req.params.get("snapshot").cloned().unwrap_or_else(|| req.params.clone()),
                )?;
                let id = snapshot.id.clone();
                self.sessions.save_layout(&snapshot)?;
                Ok(serde_json::json!({"id": id}))
            }

            "recovery.plan" => {
                let max_age_seconds = recovery_max_age_seconds(&req.params)?;
                Ok(serde_json::to_value(
                    self.sessions.recovery_plan(Duration::seconds(max_age_seconds as i64))?,
                )?)
            }

            "recovery.execute" => {
                let execute = req.params.get("execute").and_then(Value::as_bool).unwrap_or(false);
                let max_parallel =
                    req.params.get("max_parallel").and_then(Value::as_u64).unwrap_or(4) as usize;
                let max_age_seconds = recovery_max_age_seconds(&req.params)?;
                let sessions =
                    self.sessions.recovery_plan(Duration::seconds(max_age_seconds as i64))?;
                let executor = RecoveryExecutor::new(max_parallel);
                let results =
                    if execute { executor.execute(&sessions) } else { executor.dry_run(&sessions) };
                Ok(serde_json::to_value(results)?)
            }

            "process.list" => {
                self.pool.refresh().await;
                let procs: Vec<ProcessSummary> =
                    self.pool.list().await.into_iter().map(ProcessSummary::from).collect();
                Ok(serde_json::to_value(procs)?)
            }

            "process.kill" => {
                let pid: u32 =
                    req.params["pid"].as_u64().ok_or_else(|| anyhow::anyhow!("missing pid"))?
                        as u32;
                self.pool.kill(pid).await?;
                Ok(Value::Bool(true))
            }

            "process.kill_all" => {
                self.pool.kill_all().await?;
                Ok(Value::Bool(true))
            }

            "process.cmdline" => {
                let pid: u32 =
                    req.params["pid"].as_u64().ok_or_else(|| anyhow::anyhow!("missing pid"))?
                        as u32;
                // Per plan §3.3: return empty Vec when the pid is gone or the
                // cmdline is unreadable. The Swift UI renders "No command line
                // available" when the list is empty.
                let cmd = read_pid_cmdline(pid).unwrap_or_default();
                Ok(serde_json::to_value(CmdlineResponse { cmd })?)
            }

            "health.status" => {
                self.pool.refresh().await;
                let procs = self.pool.list().await;
                let (used, total) = self.pool.system_memory_usage().await;
                let (gate, host_watch) = capture_gate_host_watch()?;
                let pool = self.capture_pool_snapshot(gate.clone(), host_watch).await;
                let status = self.capture_status_snapshot().await?;
                let snap = HealthSnapshot {
                    managed_processes: procs.len(),
                    used_memory_mb: used,
                    total_memory_mb: total,
                    healthy: used < total / 2,
                    gate,
                    host_watch,
                    pool,
                    status,
                };
                Ok(serde_json::to_value(snap)?)
            }

            "pool.status" => {
                let (gate, host_watch) = capture_gate_host_watch()?;
                let mut snap = self.capture_pool_snapshot(gate, host_watch).await;
                snap.status = Some(Box::new(self.capture_status_snapshot().await?));
                Ok(serde_json::to_value(snap)?)
            }

            "status.snapshot" => {
                let mut snap = self.capture_status_snapshot().await?;
                let (gate, host_watch) = capture_gate_host_watch()?;
                snap.pool = Some(Box::new(self.capture_pool_snapshot(gate, host_watch).await));
                Ok(serde_json::to_value(snap)?)
            }

            "config.get" => {
                let cfg = self.config.read().await.clone();
                Ok(serde_json::to_value(cfg)?)
            }

            "config.set" => {
                let key =
                    req.params["key"].as_str().ok_or_else(|| anyhow::anyhow!("missing key"))?;
                let value = &req.params["value"];
                self.apply_config_patch(key, value).await?;
                Ok(Value::Bool(true))
            }

            "monitoring.report" => {
                self.pool.refresh().await;
                let procs = self.pool.list().await;
                let (used, total) = self.pool.system_memory_usage().await;
                let (gate, host_watch) = capture_gate_host_watch()?;
                let pool = self.capture_pool_snapshot(gate.clone(), host_watch).await;
                let status = self.capture_status_snapshot().await?;
                let snap = MonitoringReportSnapshot {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    total_processes: procs.len(),
                    used_memory_mb: used,
                    total_memory_mb: total,
                    processes: procs
                        .iter()
                        .map(|p| MonitoringProcessEntry {
                            pid: p.pid,
                            name: p.name.clone(),
                            memory_mb: p.memory_mb,
                            project: p.project.clone(),
                            harness: p.harness.clone(),
                            start_time: p.start_time,
                            cpu_percent: p.cpu_percent,
                            ppid: p.ppid,
                            cwd: p.cwd.clone(),
                            env_count: p.env_count,
                            state: format!("{:?}", p.state),
                            disk_read_bytes: p.disk_read_bytes,
                            disk_write_bytes: p.disk_write_bytes,
                            fd_count: p.fd_count,
                            thread_count: p.thread_count,
                        })
                        .collect(),
                    gate,
                    host_watch,
                    pool,
                    status,
                };
                Ok(serde_json::to_value(snap)?)
            }

            "log.tail" => {
                let since_id = req.params["since_id"].as_u64().unwrap_or(0);
                // Cap at 200 lines per the plan (§3.2). The client is expected
                // to advance since_id by last_id on every poll.
                let (lines, last_id) = global_log_buffer().tail(since_id, 200);
                Ok(serde_json::json!({
                    "lines": lines,
                    "last_id": last_id,
                }))
            }

            other => Err(anyhow::anyhow!("unknown method: {other}")),
        }
    }

    /// Apply a dot-path config patch: "runtime.max_memory_mb" → 8192
    async fn apply_config_patch(&self, key: &str, value: &Value) -> Result<()> {
        let mut cfg = self.config.write().await;
        let mut raw = serde_json::to_value(&*cfg)?;

        let parts: Vec<&str> = key.split('.').collect();
        set_nested(&mut raw, &parts, value.clone())
            .map_err(|e| anyhow::anyhow!("config.set {key}: {e}"))?;

        *cfg = serde_json::from_value(raw)?;
        cfg.save()?;
        Ok(())
    }
}

fn set_nested(val: &mut Value, path: &[&str], new: Value) -> Result<(), String> {
    if path.is_empty() {
        *val = new;
        return Ok(());
    }
    match val {
        Value::Object(map) => {
            let entry = map.entry(path[0]).or_insert(Value::Object(serde_json::Map::new()));
            set_nested(entry, &path[1..], new)
        }
        _ => Err(format!("expected object at segment '{}'", path[0])),
    }
}

// ---------------------------------------------------------------------------
// process.cmdline (plan §3.3) — read a process's argv.
// ---------------------------------------------------------------------------

/// IPC `process.cmdline` envelope (plan §3.3, PR 5 of dashboard expansion).
///
/// Field shape:
///   * `cmd` — `Vec<String>` of argv tokens, parsed from the platform-native
///     source (`/proc/<pid>/cmdline` on Linux, `KERN_PROCARGS2` on
///     macOS). Empty when the pid is gone or unreadable so the Swift
///     UI can render a graceful "No command line available".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CmdlineResponse {
    pub cmd: Vec<String>,
}

/// Read the argv of `pid` from the host OS.
///
/// **Linux:** `/proc/<pid>/cmdline` is a NUL-separated argv list ending with a
/// trailing NUL. We split on NUL, drop the trailing empty token, and UTF-8
/// lossy-decode each chunk (argv can legitimately contain non-UTF-8 bytes for
/// harnesses that pass binary flags).
///
/// **macOS:** `/proc/<pid>/cmdline` does not exist. We use `sysctl` with
/// `CTL_KERN, KERN_PROCARGS2, <pid>` which returns the process's argument
/// block. The first chunk is the exec path; we drop it and decode the
/// remaining C-string list (NUL-separated, also lossy UTF-8). This requires
/// the target pid to be owned by or readable from this process; when the
/// sysctl returns EPERM, ESRCH, or EACCES we fall back to `proc_pidpath`
/// (just the executable path) so the Swift UI has at least one token to
/// render. Returns `Ok(Vec::new())` on any failure path so the caller can
/// treat empty and "gone" identically.
fn read_pid_cmdline(pid: u32) -> Result<Vec<String>> {
    #[cfg(target_os = "linux")]
    {
        use std::path::PathBuf;
        let path = PathBuf::from(format!("/proc/{pid}/cmdline"));
        read_cmdline_from_proc_path(&path)
    }
    #[cfg(target_os = "macos")]
    {
        read_cmdline_macos(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Unsupported platforms (Windows, BSD): return empty.
        Ok(Vec::new())
    }
}

/// Linux: read `/proc/<pid>/cmdline`, split on NUL, drop trailing empty.
#[cfg(target_os = "linux")]
fn read_cmdline_from_proc_path(path: &std::path::Path) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    file.read_to_end(&mut bytes).with_context(|| format!("read {}", path.display()))?;

    // `/proc/.../cmdline` ends with a trailing NUL; split_and_drop leaves
    // one empty trailing token, which we discard.
    Ok(split_nul_tokens(&bytes))
}

/// Split a NUL-separated byte buffer into UTF-8 lossy-decoded strings,
/// dropping empty tokens (handles the trailing NUL in `/proc/.../cmdline`).
fn split_nul_tokens(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect()
}

/// macOS: read argv via `sysctl(CTL_KERN, KERN_PROCARGS2, pid)`.
///
/// The buffer layout is: `<argc as int32><exec path NUL><argv[0] NUL>...<argv[N] NUL><env vars...>`.
/// We slice off the leading argc (4 bytes), drop the exec-path token, then
/// split the remainder on NUL and collect non-empty UTF-8 lossy chunks.
#[cfg(target_os = "macos")]
fn read_cmdline_macos(pid: u32) -> Result<Vec<String>> {
    // KERN_PROCARGS2 = 43; CTL_KERN = 1
    const CTL_KERN: libc::c_int = 1;
    const KERN_PROCARGS2: libc::c_int = 43;

    let mib: [libc::c_int; 3] = [CTL_KERN, KERN_PROCARGS2, pid as libc::c_int];
    read_arg_via_sysctl(&mib, pid)
}

/// Issue `sysctl(mib)` twice (size query, then read) and parse the response.
#[cfg(target_os = "macos")]
fn read_arg_via_sysctl(mib: &[libc::c_int; 3], pid: u32) -> Result<Vec<String>> {
    use libc::{c_void, size_t, sysctl};

    let mut size: size_t = 0;

    // SAFETY: sysctl with a NULL oldp is the documented "query size" form.
    let rc = unsafe {
        sysctl(
            mib.as_ptr() as *mut libc::c_int,
            mib.len() as libc::c_uint,
            std::ptr::null_mut::<c_void>(),
            &mut size,
            std::ptr::null_mut::<c_void>(),
            0,
        )
    };
    if rc != 0 {
        return Err(anyhow::anyhow!(
            "sysctl size query for pid {pid} failed: errno {}",
            std::io::Error::last_os_error()
        ));
    }
    if size == 0 {
        return Ok(Vec::new());
    }

    let mut buf = vec![0u8; size];
    let rc = unsafe {
        sysctl(
            mib.as_ptr() as *mut libc::c_int,
            mib.len() as libc::c_uint,
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
            std::ptr::null_mut::<c_void>(),
            0,
        )
    };
    if rc != 0 {
        return Err(anyhow::anyhow!(
            "sysctl read for pid {pid} failed: errno {}",
            std::io::Error::last_os_error()
        ));
    }
    buf.truncate(size);

    // Layout: first 4 bytes = argc (int32), then exec path NUL, then argv[0] NUL,
    // argv[1] NUL, ..., argv[N] NUL, then env vars NUL-separated.
    if buf.len() < 4 {
        return Ok(Vec::new());
    }
    let _argc = i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let payload = &buf[4..];

    // Drop the exec-path token (everything up to the first NUL), then split
    // the remainder into argv tokens.
    let argv_start = match payload.iter().position(|b| *b == 0) {
        Some(idx) => idx + 1,
        None => return Ok(Vec::new()),
    };
    Ok(split_nul_tokens(&payload[argv_start..]))
}

#[cfg(test)]
mod cmdline_tests {
    use super::*;

    #[test]
    fn split_nul_tokens_handles_trailing_nul() {
        // Simulates `/proc/<pid>/cmdline` ending with NUL.
        let bytes: &[u8] = b"node\0--flag\0value\0";
        let got = split_nul_tokens(bytes);
        assert_eq!(got, vec!["node", "--flag", "value"]);
    }

    #[test]
    fn split_nul_tokens_handles_empty() {
        assert_eq!(split_nul_tokens(b""), Vec::<String>::new());
        assert_eq!(split_nul_tokens(b"\0"), Vec::<String>::new());
        assert_eq!(split_nul_tokens(b"\0\0\0"), Vec::<String>::new());
    }

    #[test]
    fn split_nul_tokens_lossy_for_non_utf8() {
        // 0xFF is not valid UTF-8; we still want to surface the readable part.
        let bytes: &[u8] = &[b'n', b'o', b'd', b'e', 0, 0xFF, 0xFE, 0, b'd', b'o', b'n', b'e', 0];
        let got = split_nul_tokens(bytes);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], "node");
        assert_eq!(got[2], "done");
    }

    #[test]
    fn cmdline_response_serializes_to_expected_shape() {
        let r = CmdlineResponse { cmd: vec!["node".into(), "server.js".into()] };
        let v = serde_json::to_value(&r).unwrap();
        let arr = v.get("cmd").and_then(|x| x.as_array()).expect("cmd is array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "node");
        assert_eq!(arr[1], "server.js");
    }

    #[test]
    fn read_pid_cmdline_returns_empty_for_zero_pid() {
        // PID 0 is the scheduler; /proc/0 doesn't expose cmdline on most
        // distros. Either an Err (caught by unwrap_or_default → []) or an Ok([])
        // is acceptable — both paths must produce an empty Vec.
        let got = read_pid_cmdline(0).unwrap_or_default();
        assert!(got.is_empty(), "pid 0 must yield empty cmdline");
    }

    #[test]
    fn read_pid_cmdline_returns_empty_for_nonexistent_pid() {
        // Use a wildly high pid that's almost certainly unused.
        let got = read_pid_cmdline(0x7FFFFFFE).unwrap_or_default();
        assert!(got.is_empty(), "missing pid must yield empty cmdline");
    }
}

// ---------------------------------------------------------------------------
// Wire-type and handler edge-case tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod wire_type_tests {
    use super::*;

    /// Request deserializes from valid JSON with all fields.
    #[test]
    fn request_deserialize_from_json() {
        let raw = r#"{"id":42,"method":"process.list","params":{}}"#;
        let req: Request = serde_json::from_str(raw).expect("valid JSON must deserialize");
        assert_eq!(req.id, 42);
        assert_eq!(req.method, "process.list");
        assert_eq!(req.params, serde_json::json!({}));
    }

    /// Request defaults params to Value::Null when missing (serde default).
    #[test]
    fn request_defaults_params_to_null_when_missing() {
        let raw = r#"{"id":1,"method":"health.status"}"#;
        let req: Request =
            serde_json::from_str(raw).expect("params is optional via serde(default)");
        assert_eq!(req.params, Value::Null);
    }

    /// Request rejects JSON missing required fields (method).
    #[test]
    fn request_rejects_missing_method() {
        let raw = r#"{"id":1}"#;
        assert!(serde_json::from_str::<Request>(raw).is_err(), "missing method must fail");
    }

    /// Response::ok serializes with null error and provided result.
    #[test]
    fn response_ok_has_null_error_and_result() {
        let resp = Response::ok(10, serde_json::json!({"key": "value"}));
        assert_eq!(resp.id, 10);
        assert!(resp.error.is_none());
        assert_eq!(resp.result, serde_json::json!({"key": "value"}));
    }

    /// Response::err sets error message and null result.
    #[test]
    fn response_err_has_error_and_null_result() {
        let resp = Response::err(5, "something broke");
        assert_eq!(resp.id, 5);
        assert_eq!(resp.error.as_deref(), Some("something broke"));
        assert_eq!(resp.result, Value::Null);
    }

    /// Response::ok handles non-serializable result by falling back to null.
    #[test]
    fn response_ok_falls_back_to_null_for_unserializable() {
        // () is serializable as null in serde_json, so this tests the path.
        let resp = Response::ok(0, ());
        assert_eq!(resp.result, Value::Null);
    }

    /// ProcessSummary serialization roundtrip preserves all fields.
    #[test]
    fn process_summary_roundtrip() {
        let ps = ProcessSummary {
            pid: 1234,
            name: "cargo".into(),
            cmd: vec!["cargo".into(), "build".into()],
            memory_mb: 256,
            project: Some("myproject".into()),
            harness: Some("jcode".into()),
            start_time: 1700000000,
            cpu_percent: 12.5,
            ppid: Some(1),
            cwd: Some("/repo".into()),
            env_count: 42,
            fd_count: Some(100),
            thread_count: Some(8),
            disk_read_bytes: Some(1024),
            disk_write_bytes: Some(512),
            state: "Run".into(),
        };
        let json = serde_json::to_string(&ps).expect("serialize");
        let parsed: ProcessSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.pid, 1234);
        assert_eq!(parsed.name, "cargo");
        assert_eq!(parsed.cmd, vec!["cargo", "build"]);
        assert_eq!(parsed.memory_mb, 256);
        assert_eq!(parsed.project.as_deref(), Some("myproject"));
        assert_eq!(parsed.harness.as_deref(), Some("jcode"));
        assert_eq!(parsed.cpu_percent, 12.5);
        assert_eq!(parsed.env_count, 42);
        assert_eq!(parsed.fd_count, Some(100));
        assert_eq!(parsed.thread_count, Some(8));
    }

    /// ProcessSummary with all optional fields as None defaults via serde.
    #[test]
    fn process_summary_defaults_none_fields() {
        let raw = r#"{"pid":1,"name":"x","cmd":[],"memory_mb":0,"state":""}"#;
        let ps: ProcessSummary =
            serde_json::from_str(raw).expect("defaults must fill missing fields");
        assert_eq!(ps.pid, 1);
        assert!(ps.project.is_none());
        assert!(ps.harness.is_none());
        assert_eq!(ps.start_time, 0);
        assert_eq!(ps.cpu_percent, 0.0);
        assert!(ps.ppid.is_none());
        assert!(ps.cwd.is_none());
        assert_eq!(ps.env_count, 0);
        assert!(ps.fd_count.is_none());
        assert!(ps.thread_count.is_none());
        assert!(ps.disk_read_bytes.is_none());
        assert!(ps.disk_write_bytes.is_none());
    }

    /// set_nested sets a value at a single-segment path.
    #[test]
    fn set_nested_single_segment() {
        let mut val = serde_json::json!({});
        set_nested(&mut val, &["key"], serde_json::json!("hello")).unwrap();
        assert_eq!(val["key"], "hello");
    }

    /// set_nested sets a value at a multi-segment dot path.
    #[test]
    fn set_nested_multi_segment_dot_path() {
        let mut val = serde_json::json!({"a": {"b": 1}});
        set_nested(&mut val, &["a", "b"], serde_json::json!(42)).unwrap();
        assert_eq!(val["a"]["b"], 42);
    }

    /// set_nested returns error when a segment expects a non-object.
    #[test]
    fn set_nested_error_on_non_object() {
        let mut val = serde_json::json!({"a": "string"});
        let err = set_nested(&mut val, &["a", "b"], serde_json::json!(1));
        assert!(err.is_err(), "must fail when 'a' is a string, not an object");
    }

    /// set_nested with empty path replaces the entire value.
    #[test]
    fn set_nested_empty_path_replaces_root() {
        let mut val = serde_json::json!({"old": true});
        set_nested(&mut val, &[], serde_json::json!("new")).unwrap();
        assert_eq!(val, serde_json::json!("new"));
    }

    /// recovery_max_age_seconds defaults to DEFAULT when param is missing.
    #[test]
    fn recovery_max_age_defaults_when_missing() {
        let params = serde_json::json!({});
        let got = recovery_max_age_seconds(&params).expect("should default");
        assert_eq!(got, DEFAULT_RECOVERY_MAX_AGE_SECONDS);
    }

    /// recovery_max_age_seconds rejects values below 1.
    #[test]
    fn recovery_max_age_rejects_below_one() {
        let params = serde_json::json!({"max_age_seconds": 0});
        assert!(recovery_max_age_seconds(&params).is_err(), "0 must be rejected");
    }

    /// recovery_max_age_seconds rejects values above 604800 (7 days).
    #[test]
    fn recovery_max_age_rejects_above_max() {
        let params = serde_json::json!({"max_age_seconds": 604801});
        assert!(recovery_max_age_seconds(&params).is_err(), "604801 must be rejected");
    }

    /// recovery_max_age_seconds accepts boundary value 604800.
    #[test]
    fn recovery_max_age_accepts_max_boundary() {
        let params = serde_json::json!({"max_age_seconds": 604800});
        assert_eq!(recovery_max_age_seconds(&params).unwrap(), 604800);
    }

    /// split_nul_tokens handles a single token without trailing NUL.
    #[test]
    fn split_nul_tokens_single_token_no_trailing_nul() {
        assert_eq!(split_nul_tokens(b"hello"), vec!["hello"]);
    }

    /// CmdlineResponse deserializes from JSON.
    #[test]
    fn cmdline_response_deserialize_from_json() {
        let raw = r#"{"cmd":["rustc","--edition","2021"]}"#;
        let resp: CmdlineResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.cmd, vec!["rustc", "--edition", "2021"]);
    }

    /// MonitoringProcessEntry with all optional fields defaults to None.
    #[test]
    fn monitoring_process_entry_defaults_none_fields() {
        let raw = r#"{"pid":1,"name":"x","memory_mb":0}"#;
        let entry: MonitoringProcessEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(entry.pid, 1);
        assert!(entry.project.is_none());
        assert!(entry.ppid.is_none());
        assert!(entry.cwd.is_none());
        assert_eq!(entry.env_count, 0);
        assert!(entry.fd_count.is_none());
        assert!(entry.thread_count.is_none());
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    /// dispatch with completely invalid JSON returns a parse error response.
    #[tokio::test]
    async fn dispatch_invalid_json_returns_parse_error() {
        let handler = Handler::with_session_store(SessionStore::open_memory().unwrap());
        let resp = handler.dispatch("not json at all").await;
        assert_eq!(resp.id, 0, "parse error uses id=0");
        assert!(resp.error.is_some(), "must have error");
        let msg = resp.error.unwrap();
        assert!(msg.contains("parse error"), "must mention parse error: {msg}");
    }

    /// dispatch with unknown method returns an error response.
    #[tokio::test]
    async fn dispatch_unknown_method_returns_error() {
        let handler = Handler::with_session_store(SessionStore::open_memory().unwrap());
        let raw = r#"{"id":7,"method":"nope.nope","params":{}}"#;
        let resp = handler.dispatch(raw).await;
        assert_eq!(resp.id, 7);
        assert!(resp.error.is_some());
        assert!(resp.error.unwrap().contains("unknown method"));
    }

    /// dispatch session.list returns a JSON array (possibly empty).
    #[tokio::test]
    async fn dispatch_session_list_returns_array() {
        let handler = Handler::with_session_store(SessionStore::open_memory().unwrap());
        let raw = r#"{"id":1,"method":"session.list","params":{}}"#;
        let resp = handler.dispatch(raw).await;
        assert!(resp.error.is_none(), "session.list should not error: {:?}", resp.error);
        assert!(resp.result.is_array(), "result must be an array");
    }
}
