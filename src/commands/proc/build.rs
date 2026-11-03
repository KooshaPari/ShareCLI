// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Snapshot / row builders for `sharecli proc` (FR-006).
//!
//! - [`AgentProcSnapshot::capture`] runs the host scan + watch + thermal poll
//!   for the flat inventory snapshot (AC-006.13).
//! - [`build_agent_ppid_map`] / [`build_agent_cmdline_map`] resolve agent
//!   parents and joined cmdlines for the inventory helpers
//!   (AC-006.25 / AC-006.30).
//! - [`build_proc_detail`] resolves one PID to a [`super::types::ProcDetailSnapshot`]
//!   (AC-006.23).
//! - [`agent_row_from_watch`] / [`agent_tree_node_to_json`] /
//!   [`format_cmdline`] / [`state_letter_for_pid`] / [`state_json_from_char`] /
//!   [`state_text_from_detail_state`] are the per-row and per-node JSON
//!   conversions + state-letter formatting (AC-006.32 / AC-006.33 / AC-006.34).

use std::collections::HashMap;

use anyhow::{Context, Result};
use sharecli_fleet::{
    build_agent_state_map, format_rss_bytes, gate_status_snapshot, lookup_proc, match_known_agent,
    scan_host_agents, walk_agent_ancestors, watch_detected_agents, AgentResourceSample,
    AgentTreeNode, HostProcSource, ProcSource, ThermalGovernor,
};

use crate::monitoring::HostResourceWatchJson;

use super::types::{
    AgentAncestorRef, AgentProcRow, AgentProcSnapshot, AgentTreeNodeJson, ProcDetailSnapshot,
};

/// Build one JSON/CSV agent row including process state (AC-006.32).
pub fn agent_row_from_watch(
    row: &sharecli_fleet::DetectedAgentWatch,
    state_by_pid: &HashMap<u32, char>,
) -> AgentProcRow {
    AgentProcRow {
        pid: row.agent.pid,
        family: row.agent.family.to_string(),
        comm: row.agent.comm.clone(),
        state: state_letter_for_pid(state_by_pid, row.agent.pid),
        mem_rss_bytes: row.resource.mem_rss_bytes,
        mem_rss: format_rss_bytes(row.resource.mem_rss_bytes),
        fd_count: row.resource.fd_count,
    }
}

/// Build one tree JSON node including process state (AC-006.34).
pub fn agent_tree_node_to_json(
    node: &AgentTreeNode,
    state_by_pid: &HashMap<u32, char>,
) -> AgentTreeNodeJson {
    AgentTreeNodeJson {
        pid: node.pid,
        ppid: node.ppid,
        comm: node.comm.clone(),
        family: node.family.map(str::to_string),
        state: state_letter_for_pid(state_by_pid, node.pid),
        children: node
            .children
            .iter()
            .map(|child| agent_tree_node_to_json(child, state_by_pid))
            .collect(),
    }
}

pub(crate) fn state_letter_for_pid(state_by_pid: &HashMap<u32, char>, pid: u32) -> String {
    state_by_pid.get(&pid).map(|ch| ch.to_string()).unwrap_or_default()
}

pub(crate) fn state_json_from_char(state: char) -> String {
    if state == '?' {
        String::new()
    } else {
        state.to_string()
    }
}

pub(crate) fn state_text_from_detail_state(state: &str) -> String {
    if state.is_empty() {
        "-".into()
    } else {
        state.to_string()
    }
}

/// Map agent PID → parent PID from a proc source (AC-006.25).
pub fn build_agent_ppid_map(source: &dyn ProcSource, agent_pids: &[u32]) -> HashMap<u32, u32> {
    agent_pids
        .iter()
        .filter_map(|&pid| lookup_proc(source, pid).map(|proc| (pid, proc.ppid)))
        .collect()
}

/// Map agent PID → joined argv/cmdline from a proc source (AC-006.30).
pub fn build_agent_cmdline_map(
    source: &dyn ProcSource,
    agent_pids: &[u32],
) -> HashMap<u32, String> {
    agent_pids
        .iter()
        .filter_map(|&pid| {
            lookup_proc(source, pid).map(|proc| (pid, format_cmdline(&proc.cmdline)))
        })
        .collect()
}

/// Build proc detail for one PID from a proc source (AC-006.23).
pub fn build_proc_detail(source: &dyn ProcSource, pid: u32) -> Result<ProcDetailSnapshot> {
    let proc = lookup_proc(source, pid)
        .with_context(|| format!("process {pid} not found on this host"))?;
    let parent_comm = lookup_proc(source, proc.ppid).map(|p| p.comm);
    let direct_family = match_known_agent(&proc.comm, &proc.cmdline).map(str::to_string);
    let agent_ancestor = if direct_family.is_some() {
        None
    } else {
        walk_agent_ancestors(source, pid)
            .map(|agent| AgentAncestorRef { pid: agent.pid, family: agent.family.to_string() })
    };
    let resource = AgentResourceSample::capture_for_pid(pid)
        .with_context(|| format!("failed to sample RSS/FD for process {pid}"))?;
    let agents = scan_host_agents();
    let thermal = ThermalGovernor::new().poll()?;
    let gate = gate_status_snapshot(thermal, agents.len());
    Ok(ProcDetailSnapshot {
        pid: proc.pid,
        ppid: proc.ppid,
        parent_comm,
        comm: proc.comm,
        state: state_json_from_char(proc.state),
        cmdline: proc.cmdline,
        family: direct_family,
        agent_ancestor,
        mem_rss_bytes: resource.mem_rss_bytes,
        mem_rss: format_rss_bytes(resource.mem_rss_bytes),
        fd_count: resource.fd_count,
        gate,
        host_watch: HostResourceWatchJson::capture()?,
        pool: None,
        status: None,
    })
}

pub(crate) fn format_cmdline(cmdline: &[String]) -> String {
    if cmdline.is_empty() {
        return "(empty)".into();
    }
    cmdline.join(" ")
}

impl AgentProcSnapshot {
    pub fn capture() -> Result<Self> {
        let agents = scan_host_agents();
        let watched = watch_detected_agents(&agents);
        let thermal = ThermalGovernor::new().poll()?;
        let gate = gate_status_snapshot(thermal, agents.len());
        let agent_pids: Vec<u32> = agents.iter().map(|a| a.pid).collect();
        let state_by_pid = build_agent_state_map(&HostProcSource, &agent_pids);
        Ok(Self {
            agents: watched.iter().map(|row| agent_row_from_watch(row, &state_by_pid)).collect(),
            scanned: agents.len(),
            watched: watched.len(),
            gate,
            host_watch: HostResourceWatchJson::capture()?,
            pool: None,
            status: None,
        })
    }
}
