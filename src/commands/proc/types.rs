// SPDX-License-Identifier: Apache-2.0 OR MIT
//! JSON envelope structs for `sharecli proc` (FR-006).
//!
//! Every snapshot/row/NDJSON line is declared here. Renderers
//! ([`crate::commands::proc::render`]) and the entry-point ([`crate::commands::proc::run`])
//! consume these by reference. External callers (main.rs, tests, downstream
//! `ps`/`status` paths) read the same names via the `commands::proc::*` re-exports.

use serde::{Deserialize, Serialize};

use crate::monitoring::HostResourceWatchJson;

/// One detected agent row for text/JSON surfaces (AC-006.11, AC-006.32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProcRow {
    pub pid: u32,
    pub family: String,
    pub comm: String,
    /// Linux `/proc` state letter (R|S|D|Z|T|t|…); AC-006.32.
    pub state: String,
    pub mem_rss_bytes: u64,
    pub mem_rss: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd_count: Option<u64>,
}

/// JSON payload for `sharecli proc --json` (AC-006.13, pool/status siblings AC-007.77).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentProcSnapshot {
    pub agents: Vec<AgentProcRow>,
    pub scanned: usize,
    pub watched: usize,
    pub gate: sharecli_fleet::GateStatusSnapshot,
    /// Live host FD/RSS/load/net watch (FR-007 / AC-007.13).
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<super::super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::super::StatusJson>,
}

/// JSON payload for `sharecli proc --tree --json` (AC-006.16, pool/status siblings AC-007.77).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentTreeSnapshot {
    pub forests: Vec<AgentTreeNodeJson>,
    pub roots: usize,
    /// Live thermal + agent gate snapshot (FR-007 / AC-007.18).
    pub gate: sharecli_fleet::GateStatusSnapshot,
    /// Live host FD/RSS/load/net watch (FR-007 / AC-007.15).
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<super::super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::super::StatusJson>,
}

/// Nearest ancestor agent reference for proc detail (AC-006.23).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentAncestorRef {
    pub pid: u32,
    pub family: String,
}

/// JSON payload for `sharecli proc --pid N --json` (AC-006.23).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcDetailSnapshot {
    pub pid: u32,
    pub ppid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_comm: Option<String>,
    pub comm: String,
    /// Linux `/proc` state letter (R|S|D|Z|T|t|…); AC-006.33.
    pub state: String,
    pub cmdline: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_ancestor: Option<AgentAncestorRef>,
    pub mem_rss_bytes: u64,
    pub mem_rss: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd_count: Option<u64>,
    /// Live thermal + agent gate snapshot (FR-007 / AC-007.17).
    pub gate: sharecli_fleet::GateStatusSnapshot,
    /// Live host FD/RSS/load/net watch (FR-007 / AC-007.16).
    pub host_watch: HostResourceWatchJson,
    /// Runtime pool operator panel (FR-007 / AC-007.77, proc --pid parity AC-007.86).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<super::super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77, proc --pid parity AC-007.86).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::super::StatusJson>,
}

/// One tree node in `AgentTreeSnapshot` (AC-006.16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentTreeNodeJson {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Linux `/proc` state letter (R|S|D|Z|T|t|…); AC-006.34.
    pub state: String,
    pub children: Vec<AgentTreeNodeJson>,
}

/// One NDJSON watch line for flat inventory (`proc --watch --json`, AC-006.18 / AC-006.37).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentProcNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: AgentProcSnapshot,
}

/// One NDJSON watch line for tree inventory (`proc --tree --watch --json`, AC-006.18).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentTreeNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: AgentTreeSnapshot,
}

/// One NDJSON watch line for process detail (`proc --pid N --watch --json`, AC-007.87).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcDetailNdjsonLine {
    pub ts: u64,
    #[serde(flatten)]
    pub snapshot: ProcDetailSnapshot,
}
