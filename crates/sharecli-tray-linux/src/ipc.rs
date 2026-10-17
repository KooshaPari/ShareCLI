// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Blocking Unix-socket NDJSON-RPC client for the sharecli IPC server.
//!
//! Mirrors the wire contract in `crates/sharecli-ipc/src/handler.rs` and the
//! macOS Swift `IPCClient`. Each call opens its own connection — the Rust IPC
//! server handles concurrent connections, so no shared state is kept here.
//!
//! The RPC functions are only wired into the tray on Linux (the `ksni` binary
//! target), but the module compiles everywhere so its wire types and
//! `socket_path` stay unit-testable cross-platform.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[allow(unused_imports)]
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
#[allow(unused_imports)]
use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Request<'a> {
    id: u64,
    method: &'a str,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct Response<T> {
    #[allow(dead_code)]
    id: u64,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessSummary {
    pub pid: u32,
    pub name: String,
    #[allow(dead_code)]
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    pub project: Option<String>,
    pub harness: Option<String>,
    #[allow(dead_code)]
    pub start_time: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GateStatusSnapshot {
    pub thermal_pressure: String,
    pub detected_agents: usize,
    pub agent_total_rss_bytes: u64,
    pub agent_contention: String,
    pub gate_decision: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HostResourceWatchJson {
    pub fd_count: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub mem_rss_bytes: u64,
    pub load_1m: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthSnapshot {
    pub managed_processes: usize,
    pub used_memory_mb: u64,
    pub total_memory_mb: u64,
    pub healthy: bool,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentProcRow {
    pub pid: u32,
    pub family: String,
    pub comm: String,
    pub state: String,
    pub mem_rss_bytes: u64,
    pub mem_rss: String,
    pub fd_count: Option<u64>,
}

/// IPC `pool.status` envelope (FR-007 / AC-007.67, tray wire AC-007.68).
#[derive(Debug, Clone, Deserialize)]
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
}

/// IPC `status.snapshot` envelope (FR-007 / AC-007.67, tray wire AC-007.68).
#[derive(Debug, Clone, Deserialize)]
pub struct StatusSnapshot {
    pub total_processes: usize,
    pub agents: Vec<AgentProcRow>,
    pub scanned: usize,
    pub watched: usize,
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringProcessEntry {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub project: Option<String>,
    pub harness: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

/// Resolve the IPC socket path, honoring `SHARECLI_IPC_SOCK` and falling back to
/// `$XDG_DATA_HOME/sharecli/ipc.sock` (matching `sharecli-ipc::socket_path`).
pub fn socket_path() -> PathBuf {
    if let Ok(v) = std::env::var("SHARECLI_IPC_SOCK") {
        return PathBuf::from(v);
    }
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("sharecli").join("ipc.sock")
}

#[cfg_attr(not(unix), allow(dead_code))]
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(unix)]
fn call<T: for<'de> Deserialize<'de>>(method: &str, params: serde_json::Value) -> Result<T> {
    let path = socket_path();
    let stream = UnixStream::connect(&path)
        .with_context(|| format!("connect to sharecli IPC socket at {}", path.display()))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut writer = stream.try_clone()?;
    let payload = serde_json::to_string(&Request { id, method, params })?;
    writer.write_all(payload.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.trim().is_empty() {
        return Err(anyhow!("empty response from IPC server"));
    }

    let resp: Response<T> = serde_json::from_str(line.trim())
        .with_context(|| format!("decode IPC response for {method}"))?;
    if let Some(err) = resp.error {
        return Err(anyhow!("IPC error ({method}): {err}"));
    }
    resp.result.ok_or_else(|| anyhow!("IPC response for {method} had no result"))
}

#[cfg(not(unix))]
fn call<T: for<'de> Deserialize<'de>>(_method: &str, _params: serde_json::Value) -> Result<T> {
    Err(anyhow!("IPC not supported on this platform"))
}

pub fn list_processes() -> Result<Vec<ProcessSummary>> {
    call("process.list", serde_json::json!({}))
}

pub fn health() -> Result<HealthSnapshot> {
    call("health.status", serde_json::json!({}))
}

pub fn monitoring_report() -> Result<MonitoringReportSnapshot> {
    call("monitoring.report", serde_json::json!({}))
}

pub fn pool_status() -> Result<PoolSnapshot> {
    call("pool.status", serde_json::json!({}))
}

pub fn status_snapshot() -> Result<StatusSnapshot> {
    call("status.snapshot", serde_json::json!({}))
}

impl MonitoringReportSnapshot {
    /// Map fleet monitoring snapshot → tray health fields (parity with `health.status`).
    pub fn health_snapshot(&self) -> HealthSnapshot {
        HealthSnapshot {
            managed_processes: self.total_processes,
            used_memory_mb: self.used_memory_mb,
            total_memory_mb: self.total_memory_mb,
            healthy: self.used_memory_mb < self.total_memory_mb / 2,
            gate: self.gate.clone(),
            host_watch: self.host_watch.clone(),
        }
    }

    /// Map fleet monitoring processes → tray process rows (parity with `process.list`).
    pub fn process_summaries(&self) -> Vec<ProcessSummary> {
        self.processes
            .iter()
            .map(|p| ProcessSummary {
                pid: p.pid,
                name: p.name.clone(),
                cmd: Vec::new(),
                memory_mb: p.memory_mb,
                project: p.project.clone(),
                harness: p.harness.clone(),
                start_time: 0,
            })
            .collect()
    }
}

pub fn kill(pid: u32) -> Result<bool> {
    call("process.kill", serde_json::json!({ "pid": pid }))
}

pub fn kill_all() -> Result<bool> {
    call("process.kill_all", serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_honors_env_override() {
        std::env::set_var("SHARECLI_IPC_SOCK", "/tmp/custom-sharecli.sock");
        assert_eq!(socket_path(), PathBuf::from("/tmp/custom-sharecli.sock"));
        std::env::remove_var("SHARECLI_IPC_SOCK");
    }

    #[test]
    fn socket_path_default_ends_with_ipc_sock() {
        std::env::remove_var("SHARECLI_IPC_SOCK");
        let p = socket_path();
        assert!(p.ends_with("sharecli/ipc.sock"), "unexpected default path: {}", p.display());
    }

    #[test]
    fn process_summary_matches_server_wire_shape() {
        // Byte-for-byte the JSON emitted by sharecli-ipc::handler::ProcessSummary.
        let raw = r#"{"pid":4242,"name":"claude","cmd":["claude","--foo"],
            "memory_mb":128,"project":"omniroute","harness":"claude","start_time":17}"#;
        let p: ProcessSummary = serde_json::from_str(raw).unwrap();
        assert_eq!(p.pid, 4242);
        assert_eq!(p.name, "claude");
        assert_eq!(p.memory_mb, 128);
        assert_eq!(p.project.as_deref(), Some("omniroute"));
        assert_eq!(p.harness.as_deref(), Some("claude"));
    }

    #[test]
    fn process_summary_allows_null_optionals() {
        let raw = r#"{"pid":1,"name":"node","cmd":[],"memory_mb":0,
            "project":null,"harness":null,"start_time":0}"#;
        let p: ProcessSummary = serde_json::from_str(raw).unwrap();
        assert!(p.project.is_none());
        assert!(p.harness.is_none());
    }

    #[test]
    fn health_snapshot_matches_server_wire_shape() {
        let raw = r#"{"managed_processes":3,"used_memory_mb":2048,
            "total_memory_mb":16384,"healthy":true,
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}"#;
        let h: HealthSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(h.managed_processes, 3);
        assert_eq!(h.used_memory_mb, 2048);
        assert_eq!(h.total_memory_mb, 16384);
        assert!(h.healthy);
        assert_eq!(h.gate.gate_decision, "ADMIT");
        assert_eq!(h.host_watch.load_1m, 0.5);
    }

    #[test]
    fn monitoring_report_snapshot_matches_server_wire_shape() {
        // Byte-for-byte the JSON emitted by sharecli-ipc::handler::MonitoringReportSnapshot.
        let raw = r#"{"timestamp":1700000000,"total_processes":1,"used_memory_mb":256,
            "total_memory_mb":16384,"processes":[{"pid":99,"name":"worker","memory_mb":64,
            "project":null,"harness":"native"}],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5},
            "pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
            "healthy":true,"issues":[],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}},
            "status":{"total_processes":2,"agents":[],"scanned":50,"watched":1,
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}}"#;
        let snap: MonitoringReportSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snap.timestamp, 1_700_000_000);
        assert_eq!(snap.total_processes, 1);
        assert_eq!(snap.used_memory_mb, 256);
        assert_eq!(snap.processes.len(), 1);
        assert_eq!(snap.processes[0].pid, 99);
        assert_eq!(snap.processes[0].name, "worker");
        assert_eq!(snap.gate.gate_decision, "ADMIT");
        assert_eq!(snap.host_watch.load_1m, 0.5);
        assert_eq!(snap.pool.node_total, 2);
        assert_eq!(snap.status.scanned, 50);
    }

    #[test]
    fn pool_snapshot_matches_server_wire_shape() {
        let raw = r#"{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
            "healthy":true,"issues":[],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}"#;
        let snap: PoolSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snap.node_total, 2);
        assert_eq!(snap.bun_idle, 0);
        assert!(snap.healthy);
        assert_eq!(snap.gate.gate_decision, "ADMIT");
        assert_eq!(snap.host_watch.load_1m, 0.5);
    }

    #[test]
    fn status_snapshot_matches_server_wire_shape() {
        let raw = r#"{"total_processes":2,"agents":[{"pid":99,"family":"claude","comm":"claude",
            "state":"S","mem_rss_bytes":4096,"mem_rss":"4.0M","fd_count":12}],
            "scanned":50,"watched":1,
            "gate":{"thermal_pressure":"GREEN","detected_agents":1,
            "agent_total_rss_bytes":4096,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}"#;
        let snap: StatusSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snap.total_processes, 2);
        assert_eq!(snap.agents[0].pid, 99);
        assert_eq!(snap.scanned, 50);
        assert_eq!(snap.gate.gate_decision, "ADMIT");
        assert_eq!(snap.host_watch.load_1m, 0.5);
    }

    #[test]
    fn monitoring_report_snapshot_maps_health_and_processes() {
        let raw = r#"{"timestamp":1700000000,"total_processes":2,"used_memory_mb":256,
            "total_memory_mb":16384,"processes":[{"pid":99,"name":"worker","memory_mb":64,
            "project":"demo","harness":"native"},{"pid":100,"name":"agent","memory_mb":32,
            "project":null,"harness":null}],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5},
            "pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
            "healthy":true,"issues":[],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}},
            "status":{"total_processes":2,"agents":[],"scanned":50,"watched":1,
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}}"#;
        let snap: MonitoringReportSnapshot = serde_json::from_str(raw).unwrap();
        let health = snap.health_snapshot();
        assert_eq!(health.managed_processes, 2);
        assert_eq!(health.used_memory_mb, 256);
        assert!(health.healthy);
        assert_eq!(health.gate.gate_decision, "ADMIT");
        assert_eq!(health.host_watch.load_1m, 0.5);

        let procs = snap.process_summaries();
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].pid, 99);
        assert_eq!(procs[0].name, "worker");
        assert_eq!(procs[0].memory_mb, 64);
        assert_eq!(procs[0].project.as_deref(), Some("demo"));
        assert_eq!(procs[0].harness.as_deref(), Some("native"));
        assert!(procs[0].cmd.is_empty());
    }

    #[test]
    fn response_surfaces_server_error() {
        let raw = r#"{"id":7,"result":null,"error":"boom"}"#;
        let resp: Response<bool> = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.error.as_deref(), Some("boom"));
        assert!(resp.result.is_none());
    }
}
