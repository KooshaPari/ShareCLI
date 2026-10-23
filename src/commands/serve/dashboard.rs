// SPDX-License-Identifier: Apache-2.0 OR MIT
//! WebSocket dashboard snapshot types and builder.
//!
//! `DashboardWsSnapshot` is the operator envelope streamed over `/ws`.
//! It carries parity with the CLI / IPC `pool` and `status` JSON
//! envelopes (AC-007.41 extended by AC-007.70) so the dashboard has
//! the same information that `sharecli status` and `sharecli pool`
//! would print in a terminal.

use std::collections::HashMap;

use serde::Serialize;
use sharecli_fleet::GateStatusSnapshot;

use crate::commands::proc::AgentProcSnapshot;
use crate::commands::{PoolJson, StatusJson};
use crate::monitoring::HostResourceWatchJson;
use crate::runtime::ProcessPool;

/// One managed process row in the dashboard WebSocket snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardProcessRow {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub start_time: u64,
}

/// Compact host-agent inventory for dashboard operator parity (AC-007.41).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardAgentSummary {
    pub scanned: usize,
    pub watched: usize,
    pub total_rss_bytes: u64,
    pub families: HashMap<String, usize>,
}

/// WebSocket operator envelope: gate → host_watch → pool → status → agents → processes
/// (AC-007.41 extended by AC-007.70 for pool/status IPC parity).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardWsSnapshot {
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    pub pool: PoolJson,
    pub status: StatusJson,
    pub agents: DashboardAgentSummary,
    pub processes: Vec<DashboardProcessRow>,
}

/// Build the live dashboard WebSocket snapshot (parity with CLI/IPC pool + status envelopes).
pub async fn build_dashboard_ws_snapshot() -> anyhow::Result<DashboardWsSnapshot> {
    let managed_pool = ProcessPool::new();
    let (procs, pool_json, status_json) = tokio::join!(
        managed_pool.list(),
        crate::commands::build_pool_json(),
        crate::commands::build_status_json(),
    );
    let agent_snap = AgentProcSnapshot::capture()?;

    let mut families: HashMap<String, usize> = HashMap::new();
    let mut total_rss_bytes = 0u64;
    for row in &agent_snap.agents {
        *families.entry(row.family.clone()).or_insert(0) += 1;
        total_rss_bytes += row.mem_rss_bytes;
    }

    let processes = procs
        .iter()
        .map(|p| DashboardProcessRow {
            pid: p.pid,
            name: p.name.clone(),
            cmd: p.cmd.clone(),
            memory_mb: p.memory_mb,
            project: p.project.clone(),
            harness: p.harness.clone(),
            start_time: p.start_time,
        })
        .collect();

    Ok(DashboardWsSnapshot {
        gate: agent_snap.gate,
        host_watch: agent_snap.host_watch,
        pool: pool_json?,
        status: status_json?,
        agents: DashboardAgentSummary {
            scanned: agent_snap.scanned,
            watched: agent_snap.watched,
            total_rss_bytes,
            families,
        },
        processes,
    })
}
