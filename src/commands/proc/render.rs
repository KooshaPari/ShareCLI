// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Text / CSV renderers for `sharecli proc` (FR-006).
//!
//! - [`render_agent_inventory`] / [`render_agent_tree`] /
//!   [`render_agent_inventory_csv`] / [`render_agent_tree_csv`] /
//!   [`render_proc_detail_text`] / [`render_proc_detail_csv`] are the
//!   one-shot text and CSV renderers (AC-006.16 / AC-006.23 / AC-006.24 /
//!   AC-006.26 / AC-006.32–34 / AC-007.86).
//! - [`csv_escape_field`] is the RFC 4180-style CSV escaper used by every
//!   `proc` CSV renderer.
//! - [`append_tree_csv_row`] / [`render_tree_node`] are the recursive CSV
//!   and text-tree helpers invoked by their parent renderers.

use std::collections::HashMap;

use anyhow::Result;
use sharecli_fleet::{
    format_gate_status_from_snapshot, format_rss_bytes, state_text_for_pid, AgentTreeNode,
    DetectedAgentWatch,
};

use super::build::{format_cmdline, state_letter_for_pid, state_text_from_detail_state};
use super::types::ProcDetailSnapshot;

/// Escape one CSV field (RFC 4180-style quoting when needed).
pub fn csv_escape_field(raw: &str) -> String {
    if raw.contains(',') || raw.contains('"') || raw.contains('\n') || raw.contains('\r') {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw.to_string()
    }
}

fn append_tree_csv_row(
    out: &mut String,
    root_index: usize,
    depth: u32,
    node: &AgentTreeNode,
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) {
    let rss = rss_by_pid.get(&node.pid).copied().unwrap_or(0);
    let fd = fd_by_pid.get(&node.pid).map(|n| n.to_string()).unwrap_or_default();
    let family = node.family.map(csv_escape_field).unwrap_or_default();
    let state = state_letter_for_pid(state_by_pid, node.pid);
    out.push_str(&format!(
        "{root_index},{depth},{pid},{ppid},{family},{comm},{state},{rss},{mem_rss},{fd}\n",
        root_index = root_index,
        depth = depth,
        pid = node.pid,
        ppid = node.ppid,
        family = family,
        comm = csv_escape_field(&node.comm),
        state = state,
        rss = rss,
        mem_rss = csv_escape_field(&format_rss_bytes(rss)),
        fd = fd,
    ));
    for child in &node.children {
        append_tree_csv_row(out, root_index, depth + 1, child, rss_by_pid, fd_by_pid, state_by_pid);
    }
}

/// Render agent process forests as CSV (AC-006.26, AC-006.32).
pub fn render_agent_tree_csv(
    forests: &[AgentTreeNode],
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) -> String {
    const HEADER: &str =
        "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count";
    let mut out = String::from(HEADER);
    out.push('\n');
    for (root_index, root) in forests.iter().enumerate() {
        append_tree_csv_row(&mut out, root_index, 0, root, rss_by_pid, fd_by_pid, state_by_pid);
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Render flat agent inventory as CSV (AC-006.24, AC-006.32).
pub fn render_agent_inventory_csv(
    watched: &[DetectedAgentWatch],
    state_by_pid: &HashMap<u32, char>,
) -> String {
    const HEADER: &str = "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count";
    let mut out = String::from(HEADER);
    for row in watched {
        let fd = row.resource.fd_count.map(|n| n.to_string()).unwrap_or_default();
        let state = state_letter_for_pid(state_by_pid, row.agent.pid);
        out.push('\n');
        out.push_str(&format!(
            "{},{},{},{},{},{},{}",
            row.agent.pid,
            csv_escape_field(row.agent.family),
            csv_escape_field(&row.agent.comm),
            state,
            row.resource.mem_rss_bytes,
            csv_escape_field(&format_rss_bytes(row.resource.mem_rss_bytes)),
            fd,
        ));
    }
    out.push('\n');
    out
}

/// Render host agent inventory (text mode, AC-006.33).
pub fn render_agent_inventory(
    watched: &[DetectedAgentWatch],
    scanned: usize,
    state_by_pid: &HashMap<u32, char>,
) {
    println!("=== Host agents (proc scan) ===\n");
    if watched.is_empty() {
        println!("No known agent processes detected on this host.");
        if scanned > 0 {
            println!("\n({scanned} agent(s) omitted — process exited before resource sample)");
        }
        return;
    }
    println!("{:<8} {:<16} {:<6} {:<10} {:<8} COMM", "PID", "FAMILY", "STATE", "RSS", "FD");
    println!("{}", "-".repeat(64));
    for row in watched {
        let fd = row.resource.fd_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
        let state = state_text_for_pid(state_by_pid, row.agent.pid);
        println!(
            "{:<8} {:<16} {:<6} {:<10} {:<8} {}",
            row.agent.pid,
            row.agent.family,
            state,
            format_rss_bytes(row.resource.mem_rss_bytes),
            fd,
            row.agent.comm
        );
    }
    if watched.len() < scanned {
        println!(
            "\n({} agent(s) omitted — process exited before resource sample)",
            scanned - watched.len()
        );
    }
    println!("\nTotal: {} agent process(es)", watched.len());
}

fn render_tree_node(
    node: &AgentTreeNode,
    prefix: &str,
    is_last: bool,
    state_by_pid: &HashMap<u32, char>,
) {
    let connector = if prefix.is_empty() {
        String::new()
    } else if is_last {
        "└── ".to_string()
    } else {
        "├── ".to_string()
    };
    let family = node.family.map(|f| format!("{f} ")).unwrap_or_default();
    let state = state_text_for_pid(state_by_pid, node.pid);
    println!("{prefix}{connector}[{pid}] {state} {family}{comm}", pid = node.pid, comm = node.comm);
    let child_prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}{}", prefix, if is_last { "    " } else { "│   " })
    };
    for (i, child) in node.children.iter().enumerate() {
        render_tree_node(child, &child_prefix, i + 1 == node.children.len(), state_by_pid);
    }
}

/// Render parent-child agent process forests (text mode, AC-006.16, AC-006.34).
pub fn render_agent_tree(forests: &[AgentTreeNode], state_by_pid: &HashMap<u32, char>) {
    println!("=== Agent process tree (proc scan) ===\n");
    if forests.is_empty() {
        println!("No known agent processes detected on this host.");
        return;
    }
    for (i, root) in forests.iter().enumerate() {
        if i > 0 {
            println!();
        }
        render_tree_node(root, "", true, state_by_pid);
    }
    println!("\nTotal: {} agent root(s)", forests.len());
}

/// Render one process detail snapshot as text (AC-006.23).
/// One-shot `proc --pid N` text MUST NOT print gate/host_watch stderr companions (AC-007.34);
/// gate/host_watch stay in text sections on stdout only (AC-007.17).
pub fn render_proc_detail_text(detail: &ProcDetailSnapshot) -> Result<()> {
    println!("=== Process detail (PID {}) ===\n", detail.pid);
    println!("PID:       {}", detail.pid);
    let parent = match (&detail.parent_comm, detail.ppid) {
        (Some(comm), ppid) => format!("{ppid} ({comm})"),
        (None, 0) => "0".into(),
        (None, ppid) => ppid.to_string(),
    };
    println!("Parent:    {parent}");
    println!("COMM:      {}", detail.comm);
    println!("State:     {}", state_text_from_detail_state(&detail.state));
    println!("CMDLINE:   {}", format_cmdline(&detail.cmdline));
    if let Some(ref family) = detail.family {
        println!("Family:    {family}");
    } else if let Some(ref ancestor) = detail.agent_ancestor {
        println!("Agent:     {} (pid {})", ancestor.family, ancestor.pid);
    }
    println!("RSS:       {} ({} bytes)", detail.mem_rss, detail.mem_rss_bytes);
    let fd = detail.fd_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
    println!("FD:        {fd}");
    print!("{}", format_gate_status_from_snapshot(&detail.gate));
    print!("{}", detail.host_watch.format_text_section());
    Ok(())
}

/// Render one process detail row as CSV body (FR-007 / AC-007.86).
pub fn render_proc_detail_csv(detail: &ProcDetailSnapshot) -> String {
    const HEADER: &str = "pid,ppid,comm,state,mem_rss_bytes,mem_rss,fd_count";
    let fd = detail.fd_count.map(|n| n.to_string()).unwrap_or_default();
    format!(
        "{HEADER}\n{pid},{ppid},{comm},{state},{rss},{mem_rss},{fd}\n",
        pid = detail.pid,
        ppid = detail.ppid,
        comm = csv_escape_field(&detail.comm),
        state = csv_escape_field(&detail.state),
        rss = detail.mem_rss_bytes,
        mem_rss = csv_escape_field(&detail.mem_rss),
        fd = fd,
    )
}
