//! Wire types and mapping helpers for Windows tray IPC (AC-007.51 / AC-007.54).
//!
//! Mirrors `sharecli-tray-linux/src/ipc.rs` mapping + kill contract; the WinUI tray uses
//! equivalent C# helpers in `windows/ShareCLITray/`.

use serde::Deserialize;

/// IPC method for terminating one managed process (parity with Linux/Swift tray kill).
pub const IPC_METHOD_KILL: &str = "process.kill";

/// IPC method for terminating all managed processes (parity with Linux/Swift tray kill_all).
pub const IPC_METHOD_KILL_ALL: &str = "process.kill_all";

/// Build NDJSON-RPC request body for `process.kill` (WinUI + integration tests).
pub fn kill_request_json(id: u64, pid: u32) -> String {
    serde_json::json!({ "id": id, "method": IPC_METHOD_KILL, "params": { "pid": pid } }).to_string()
}

/// Build NDJSON-RPC request body for `process.kill_all` (WinUI + integration tests).
pub fn kill_all_request_json(id: u64) -> String {
    serde_json::json!({ "id": id, "method": IPC_METHOD_KILL_ALL, "params": {} }).to_string()
}

#[derive(Debug, Clone, Deserialize)]
/// One process row returned by `process.list`.
pub struct ProcessSummary {
    /// Process id.
    pub pid: u32,
    /// Display name (harness/project-derived when available).
    pub name: String,
    /// Full argv (deserialized for completeness; not rendered in tray).
    #[allow(dead_code)]
    pub cmd: Vec<String>,
    /// Resident memory in megabytes.
    pub memory_mb: u64,
    /// Project label when known.
    pub project: Option<String>,
    /// Harness label when known.
    pub harness: Option<String>,
    /// Epoch milliseconds at which the process started.
    #[allow(dead_code)]
    pub start_time: u64,
}

#[derive(Debug, Clone, Deserialize)]
/// Snapshot of the thermal/agent gate as projected by `health.status`.
pub struct GateStatusSnapshot {
    /// Pressure label (e.g. `"nominal"`, `"warning"`, `"critical"`).
    pub thermal_pressure: String,
    /// Count of detected agent processes.
    pub detected_agents: usize,
    /// Sum of agent RSS in bytes.
    pub agent_total_rss_bytes: u64,
    /// Contention label (e.g. `"idle"`, `"busy"`, `"saturated"`).
    pub agent_contention: String,
    /// Decision label (e.g. `"allow"`, `"throttle"`, `"block"`).
    pub gate_decision: String,
}

#[derive(Debug, Clone, Deserialize)]
/// Host-level resource counters (subset of `/proc`/`vm_stat`).
pub struct HostResourceWatchJson {
    /// Open file-descriptor count.
    pub fd_count: u64,
    /// Cumulative network RX bytes.
    pub net_rx_bytes: u64,
    /// Cumulative network TX bytes.
    pub net_tx_bytes: u64,
    /// Process RSS in bytes (host scope).
    pub mem_rss_bytes: u64,
    /// 1-minute load average.
    pub load_1m: f64,
}

#[derive(Debug, Clone, Deserialize)]
/// Aggregate health snapshot returned by `health.status`.
pub struct HealthSnapshot {
    /// Count of managed processes.
    pub managed_processes: usize,
    /// Used memory across managed processes (MB).
    pub used_memory_mb: u64,
    /// Total host memory (MB).
    pub total_memory_mb: u64,
    /// True when all subsystems report nominal.
    pub healthy: bool,
    /// Embedded gate snapshot.
    pub gate: GateStatusSnapshot,
    /// Embedded host resource counters.
    pub host_watch: HostResourceWatchJson,
}

#[derive(Debug, Clone, Deserialize)]
/// One row in the agent-process table.
pub struct AgentProcRow {
    /// Process id.
    pub pid: u32,
    /// Agent family (e.g. `"codex"`, `"claude"`).
    pub family: String,
    /// Command name.
    pub comm: String,
    /// Process state label.
    pub state: String,
    /// Resident memory in bytes.
    pub mem_rss_bytes: u64,
    /// Pretty-formatted RSS string (e.g. `"128 MiB"`).
    pub mem_rss: String,
    /// Optional file-descriptor count.
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
/// One row in the monitoring report's process list.
pub struct MonitoringProcessEntry {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub project: Option<String>,
    pub harness: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Aggregate report returned by `monitoring.report` (gate + host + pool + status).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitoring_report_snapshot_matches_server_wire_shape() {
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
        assert_eq!(snap.gate.gate_decision, "ADMIT");
        assert_eq!(snap.host_watch.load_1m, 0.5);
        assert_eq!(snap.pool.node_total, 2);
        assert_eq!(snap.status.scanned, 50);
    }

    #[test]
    fn kill_request_json_matches_linux_wire_shape() {
        let raw = kill_request_json(7, 4242);
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["method"], "process.kill");
        assert_eq!(v["params"]["pid"], 4242);
    }

    #[test]
    fn kill_all_request_json_matches_linux_wire_shape() {
        let raw = kill_all_request_json(8);
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["id"], 8);
        assert_eq!(v["method"], "process.kill_all");
        assert_eq!(v["params"], serde_json::json!({}));
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
        assert_eq!(health.gate.gate_decision, "ADMIT");
        assert_eq!(health.host_watch.load_1m, 0.5);
        assert_eq!(snap.pool.node_total, 2);
        assert_eq!(snap.status.scanned, 50);

        let procs = snap.process_summaries();
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].pid, 99);
        assert_eq!(procs[0].project.as_deref(), Some("demo"));
        assert_eq!(procs[0].harness.as_deref(), Some("native"));
        assert!(procs[1].harness.is_none());
    }
}
