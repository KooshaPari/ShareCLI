//! Filter + sort implementation for `sharecli proc` inventory (AC-006.17–AC-006.41).
//!
//! [`filter_watched_agents`] / [`filter_agent_forests`] apply a [`ProcFilter`]
//! to flat and tree-rooted inventories. [`sort_watched_agents`] /
//! [`sort_agent_forests`] apply a [`ProcSort`] with live RSS/FD/state samples.
//! [`apply_sort_watched`] / [`apply_sort_forests`] are the `Option<ProcSort>`-aware
//! wrappers used by [`crate::commands::proc::run::render_once`].

use std::collections::HashMap;

use sharecli_fleet::{AgentTreeNode, DetectedAgentWatch};

use super::filter::ProcFilter;
use super::sort::{state_sort_letter, ProcSort};

/// Apply inventory filters to watched agent rows.
pub fn filter_watched_agents(
    watched: &[DetectedAgentWatch],
    filter: &ProcFilter,
    ppid_by_pid: &HashMap<u32, u32>,
    cmdline_by_pid: &HashMap<u32, String>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<DetectedAgentWatch> {
    if !filter.active() {
        return watched.to_vec();
    }
    watched
        .iter()
        .filter(|row| {
            agent_row_matches_filter(row, filter, ppid_by_pid, cmdline_by_pid, state_by_pid)
        })
        .cloned()
        .collect()
}

fn agent_row_matches_filter(
    row: &DetectedAgentWatch,
    filter: &ProcFilter,
    ppid_by_pid: &HashMap<u32, u32>,
    cmdline_by_pid: &HashMap<u32, String>,
    state_by_pid: &HashMap<u32, char>,
) -> bool {
    if let Some(ref family) = filter.family {
        if !row.agent.family.eq_ignore_ascii_case(family) {
            return false;
        }
    }
    if let Some(ref exclude) = filter.exclude_family {
        if row.agent.family.eq_ignore_ascii_case(exclude) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.comm {
        if !super::filter::comm_matches_pattern(&row.agent.comm, pattern) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.cmdline {
        let joined = cmdline_by_pid.get(&row.agent.pid).map(String::as_str).unwrap_or("");
        if !super::filter::cmdline_matches_pattern(joined, pattern) {
            return false;
        }
    }
    if let Some(target_state) = filter.state {
        if state_by_pid.get(&row.agent.pid).copied() != Some(target_state) {
            return false;
        }
    }
    if let Some(min) = filter.min_rss_bytes {
        if row.resource.mem_rss_bytes < min {
            return false;
        }
    }
    if let Some(max) = filter.max_rss_bytes {
        if row.resource.mem_rss_bytes > max {
            return false;
        }
    }
    let fd = row.resource.fd_count.unwrap_or(0);
    if let Some(min) = filter.min_fd_count {
        if fd < min {
            return false;
        }
    }
    if let Some(max) = filter.max_fd_count {
        if fd > max {
            return false;
        }
    }
    if let Some(target_ppid) = filter.ppid {
        if ppid_by_pid.get(&row.agent.pid).copied().unwrap_or(0) != target_ppid {
            return false;
        }
    }
    true
}

pub(crate) fn rss_map_from_watched(watched: &[DetectedAgentWatch]) -> HashMap<u32, u64> {
    watched.iter().map(|row| (row.agent.pid, row.resource.mem_rss_bytes)).collect()
}

pub(crate) fn fd_map_from_watched(watched: &[DetectedAgentWatch]) -> HashMap<u32, u64> {
    watched.iter().map(|row| (row.agent.pid, row.resource.fd_count.unwrap_or(0))).collect()
}

pub(crate) fn apply_sort_watched(
    watched: Vec<DetectedAgentWatch>,
    sort: Option<ProcSort>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<DetectedAgentWatch> {
    match sort {
        Some(key) => sort_watched_agents(&watched, key, state_by_pid),
        None => watched,
    }
}

pub(crate) fn apply_sort_forests(
    forests: Vec<AgentTreeNode>,
    sort: Option<ProcSort>,
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<AgentTreeNode> {
    match sort {
        Some(key) => sort_agent_forests(&forests, key, rss_by_pid, fd_by_pid, state_by_pid),
        None => forests,
    }
}

/// Apply filters to agent-rooted forests (family on root; RSS/FD bounds via live samples).
pub fn filter_agent_forests(
    forests: &[AgentTreeNode],
    filter: &ProcFilter,
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    cmdline_by_pid: &HashMap<u32, String>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<AgentTreeNode> {
    if !filter.active() {
        return forests.to_vec();
    }
    forests
        .iter()
        .filter(|root| {
            forest_root_matches_filter(
                root,
                filter,
                rss_by_pid,
                fd_by_pid,
                cmdline_by_pid,
                state_by_pid,
            )
        })
        .cloned()
        .collect()
}

fn forest_root_matches_filter(
    root: &AgentTreeNode,
    filter: &ProcFilter,
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    cmdline_by_pid: &HashMap<u32, String>,
    state_by_pid: &HashMap<u32, char>,
) -> bool {
    if let Some(ref family) = filter.family {
        let Some(root_family) = root.family else {
            return false;
        };
        if !root_family.eq_ignore_ascii_case(family) {
            return false;
        }
    }
    if let Some(ref exclude) = filter.exclude_family {
        if root.family.is_some_and(|f| f.eq_ignore_ascii_case(exclude)) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.comm {
        if !super::filter::comm_matches_pattern(&root.comm, pattern) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.cmdline {
        let joined = cmdline_by_pid.get(&root.pid).map(String::as_str).unwrap_or("");
        if !super::filter::cmdline_matches_pattern(joined, pattern) {
            return false;
        }
    }
    if let Some(target_state) = filter.state {
        if state_by_pid.get(&root.pid).copied() != Some(target_state) {
            return false;
        }
    }
    if let Some(min) = filter.min_rss_bytes {
        let rss = rss_by_pid.get(&root.pid).copied().unwrap_or(0);
        if rss < min {
            return false;
        }
    }
    if let Some(max) = filter.max_rss_bytes {
        let rss = rss_by_pid.get(&root.pid).copied().unwrap_or(0);
        if rss > max {
            return false;
        }
    }
    let fd = fd_by_pid.get(&root.pid).copied().unwrap_or(0);
    if let Some(min) = filter.min_fd_count {
        if fd < min {
            return false;
        }
    }
    if let Some(max) = filter.max_fd_count {
        if fd > max {
            return false;
        }
    }
    if let Some(target_ppid) = filter.ppid {
        if root.ppid != target_ppid {
            return false;
        }
    }
    true
}

/// Order watched agent rows for text/JSON inventory (`--sort`, AC-006.19, AC-006.36, AC-006.41).
pub fn sort_watched_agents(
    watched: &[DetectedAgentWatch],
    sort: ProcSort,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<DetectedAgentWatch> {
    let mut rows = watched.to_vec();
    match sort {
        ProcSort::Pid => rows.sort_by_key(|row| row.agent.pid),
        ProcSort::Rss => {
            rows.sort_by(|a, b| {
                b.resource
                    .mem_rss_bytes
                    .cmp(&a.resource.mem_rss_bytes)
                    .then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Fd => {
            rows.sort_by(|a, b| {
                let fd_a = a.resource.fd_count.unwrap_or(0);
                let fd_b = b.resource.fd_count.unwrap_or(0);
                fd_b.cmp(&fd_a).then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::State => {
            rows.sort_by(|a, b| {
                state_sort_letter(state_by_pid, a.agent.pid)
                    .cmp(&state_sort_letter(state_by_pid, b.agent.pid))
                    .then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Cpu => {
            // Descending CPU usage percent; missing data (0.0) sorts last via PID tie-break.
            rows.sort_by(|a, b| {
                let cpu_a = super::platform::sample_pid_cpu_percent(a.agent.pid);
                let cpu_b = super::platform::sample_pid_cpu_percent(b.agent.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Age => {
            // Ascending age in seconds (oldest first); u64::MAX (missing data) sorts last.
            rows.sort_by(|a, b| {
                let age_a = super::platform::sample_pid_age_secs(a.agent.pid);
                let age_b = super::platform::sample_pid_age_secs(b.agent.pid);
                age_b.cmp(&age_a).then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Name => {
            rows.sort_by(|a, b| {
                a.agent.comm.cmp(&b.agent.comm).then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
    }
    rows
}

/// Order tree root forests by live RSS/FD/PID/state samples (`--sort`, AC-006.19, AC-006.36, AC-006.41).
pub fn sort_agent_forests(
    forests: &[AgentTreeNode],
    sort: ProcSort,
    rss_by_pid: &HashMap<u32, u64>,
    fd_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<AgentTreeNode> {
    let mut roots = forests.to_vec();
    match sort {
        ProcSort::Pid => roots.sort_by_key(|node| node.pid),
        ProcSort::Rss => {
            roots.sort_by(|a, b| {
                let rss_a = rss_by_pid.get(&a.pid).copied().unwrap_or(0);
                let rss_b = rss_by_pid.get(&b.pid).copied().unwrap_or(0);
                rss_b.cmp(&rss_a).then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Fd => {
            roots.sort_by(|a, b| {
                let fd_a = fd_by_pid.get(&a.pid).copied().unwrap_or(0);
                let fd_b = fd_by_pid.get(&b.pid).copied().unwrap_or(0);
                fd_b.cmp(&fd_a).then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::State => {
            roots.sort_by(|a, b| {
                state_sort_letter(state_by_pid, a.pid)
                    .cmp(&state_sort_letter(state_by_pid, b.pid))
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Cpu => {
            roots.sort_by(|a, b| {
                let cpu_a = super::platform::sample_pid_cpu_percent(a.pid);
                let cpu_b = super::platform::sample_pid_cpu_percent(b.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Age => {
            roots.sort_by(|a, b| {
                let age_a = super::platform::sample_pid_age_secs(a.pid);
                let age_b = super::platform::sample_pid_age_secs(b.pid);
                age_b.cmp(&age_a).then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Name => {
            roots.sort_by(|a, b| a.comm.cmp(&b.comm).then_with(|| a.pid.cmp(&b.pid)));
        }
    }
    roots
}
