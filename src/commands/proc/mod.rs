//! FR-006 — `sharecli proc` host agent inventory CLI.

use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sharecli_fleet::thermal::ThermalGovernor;
pub use sharecli_fleet::{build_agent_state_map, build_forest_state_map, state_text_for_pid};
use sharecli_fleet::{
    build_host_agent_forests, format_gate_status_from_snapshot, format_gate_status_section,
    format_rss_bytes, gate_status_snapshot, lookup_proc, match_known_agent, parse_rss_bytes,
    scan_host_agents, walk_agent_ancestors, watch_detected_agents, AgentResourceSample,
    AgentTreeNode, DetectedAgentWatch, HostProcSource, ProcSource, ThermalLevel,
};
use tokio::time::sleep;

use crate::monitoring::HostResourceWatchJson;

/// CSV `#` comment line separating each `--csv --watch` refresh frame (AC-007.88).
pub const PROC_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-proc-watch-frame";
/// CSV `#` comment line separating each `proc --pid --csv --watch` refresh frame (AC-007.91).
pub const PROC_PID_CSV_WATCH_FRAME_MARKER: &str = "# sharecli-proc-pid-watch-frame";

/// Inventory filter for `sharecli proc` (AC-006.17, AC-006.25, AC-006.27, AC-006.28, AC-006.29, AC-006.30, AC-006.31, AC-006.38).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcFilter {
    pub family: Option<String>,
    pub exclude_family: Option<String>,
    pub comm: Option<String>,
    pub cmdline: Option<String>,
    pub state: Option<char>,
    pub min_rss_bytes: Option<u64>,
    pub max_rss_bytes: Option<u64>,
    pub min_fd_count: Option<u64>,
    pub max_fd_count: Option<u64>,
    pub ppid: Option<u32>,
}

/// Case-insensitive substring match on process COMM (AC-006.29).
pub fn comm_matches_pattern(comm: &str, pattern: &str) -> bool {
    substring_matches_pattern(comm, pattern)
}

/// Case-insensitive substring match on joined argv/cmdline (AC-006.30).
pub fn cmdline_matches_pattern(cmdline: &str, pattern: &str) -> bool {
    substring_matches_pattern(cmdline, pattern)
}

fn substring_matches_pattern(haystack: &str, pattern: &str) -> bool {
    haystack.to_ascii_lowercase().contains(&pattern.to_ascii_lowercase())
}

/// Parse `--state <R|S|D|Z|…>` (AC-006.31).
pub fn parse_proc_state(raw: &str) -> Result<char> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("--state must not be empty");
    }
    if trimmed.len() != 1 {
        bail!("invalid --state value '{raw}'; expected single process state letter (R|S|D|Z|T|…)");
    }
    let ch = trimmed.chars().next().expect("single-char --state value (length validated above)");
    let normalized = match ch {
        'r' | 'R' => 'R',
        's' | 'S' => 'S',
        'd' | 'D' => 'D',
        'z' | 'Z' => 'Z',
        'T' => 'T',
        't' => 't',
        'X' => 'X',
        'x' => 'x',
        'k' | 'K' => 'K',
        'w' | 'W' => 'W',
        'p' | 'P' => 'P',
        'i' | 'I' => 'I',
        other => bail!("invalid --state value '{other}'; expected R, S, D, Z, T, t, …"),
    };
    Ok(normalized)
}

/// Parse `--min-fd` / `--max-fd` count (non-negative integer).
pub fn parse_fd_count(raw: &str, flag: &str) -> Result<u64> {
    let value = raw.parse::<u64>().with_context(|| format!("invalid {flag} value '{raw}'"))?;
    Ok(value)
}

impl ProcFilter {
    // CLI flag aggregation: every `proc` inventory filter is threaded through as
    // an individual Option so the dispatch layer stays mechanical. The wide
    // signature is intentional (mirrors `reject_pid_inventory_combos` / `run`).
    #[allow(clippy::too_many_arguments)]
    pub fn from_cli(
        family: Option<String>,
        exclude_family: Option<String>,
        comm: Option<String>,
        cmdline: Option<String>,
        state: Option<String>,
        min_rss: Option<String>,
        max_rss: Option<String>,
        min_fd: Option<String>,
        max_fd: Option<String>,
        ppid: Option<u32>,
    ) -> Result<Self> {
        let comm = match comm {
            None => None,
            Some(raw) if raw.is_empty() => bail!("--comm pattern must not be empty"),
            Some(raw) => Some(raw),
        };
        let cmdline = match cmdline {
            None => None,
            Some(raw) if raw.is_empty() => bail!("--cmdline pattern must not be empty"),
            Some(raw) => Some(raw),
        };
        let state = match state {
            None => None,
            Some(raw) => Some(parse_proc_state(&raw)?),
        };
        let min_rss_bytes = match min_rss {
            None => None,
            Some(raw) => Some(parse_rss_bytes(&raw, "--min-rss")?),
        };
        let max_rss_bytes = match max_rss {
            None => None,
            Some(raw) => Some(parse_rss_bytes(&raw, "--max-rss")?),
        };
        if let (Some(min), Some(max)) = (min_rss_bytes, max_rss_bytes) {
            if min > max {
                bail!("--min-rss MUST NOT exceed --max-rss");
            }
        }
        let min_fd_count = match min_fd {
            None => None,
            Some(raw) => Some(parse_fd_count(&raw, "--min-fd")?),
        };
        let max_fd_count = match max_fd {
            None => None,
            Some(raw) => Some(parse_fd_count(&raw, "--max-fd")?),
        };
        if let (Some(min), Some(max)) = (min_fd_count, max_fd_count) {
            if min > max {
                bail!("--min-fd MUST NOT exceed --max-fd");
            }
        }
        if family.is_some() && exclude_family.is_some() {
            bail!("--family and --exclude-family are mutually exclusive");
        }
        Ok(Self {
            family,
            exclude_family,
            comm,
            cmdline,
            state,
            min_rss_bytes,
            max_rss_bytes,
            min_fd_count,
            max_fd_count,
            ppid,
        })
    }

    fn active(&self) -> bool {
        self.family.is_some()
            || self.exclude_family.is_some()
            || self.comm.is_some()
            || self.cmdline.is_some()
            || self.state.is_some()
            || self.min_rss_bytes.is_some()
            || self.max_rss_bytes.is_some()
            || self.min_fd_count.is_some()
            || self.max_fd_count.is_some()
            || self.ppid.is_some()
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

/// Sort key for `sharecli proc` inventory rows (AC-006.19, AC-006.41).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcSort {
    /// Ascending PID (lowest first).
    #[default]
    Pid,
    /// Descending resident memory (highest first); PID tie-break ascending.
    Rss,
    /// Descending open FD count (highest first; missing FD treated as 0); PID tie-break.
    Fd,
    /// Ascending process state letter; missing state sorts last; PID tie-break.
    State,
    /// Descending CPU usage in percent (one-shot approximation); missing sorts last.
    Cpu,
    /// Ascending process age in seconds (oldest first); missing sorts last.
    Age,
    /// Ascending COMM name (alphabetical); PID tie-break ascending.
    Name,
}

/// Parse `--limit N` for proc inventory (AC-006.21).
pub fn parse_proc_limit(raw: Option<u64>) -> Result<Option<usize>> {
    match raw {
        None => Ok(None),
        Some(0) => bail!("--limit must be >= 1"),
        Some(n) => Ok(Some(n as usize)),
    }
}

/// Cap flat inventory rows after filter/sort (`--limit`, AC-006.21).
pub fn limit_watched_agents(
    watched: Vec<DetectedAgentWatch>,
    limit: Option<usize>,
) -> Vec<DetectedAgentWatch> {
    match limit {
        None => watched,
        Some(max) => watched.into_iter().take(max).collect(),
    }
}

/// Cap tree root forests after filter/sort (`--limit`, AC-006.21).
pub fn limit_agent_forests(
    forests: Vec<AgentTreeNode>,
    limit: Option<usize>,
) -> Vec<AgentTreeNode> {
    match limit {
        None => forests,
        Some(max) => forests.into_iter().take(max).collect(),
    }
}

impl ProcSort {
    pub fn from_cli(raw: Option<&str>) -> Result<Option<Self>> {
        match raw {
            None => Ok(None),
            Some(s) => Ok(Some(s.parse()?)),
        }
    }
}

impl std::str::FromStr for ProcSort {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "pid" => Ok(Self::Pid),
            "rss" => Ok(Self::Rss),
            "fd" => Ok(Self::Fd),
            "state" => Ok(Self::State),
            "cpu" => Ok(Self::Cpu),
            "age" => Ok(Self::Age),
            "name" => Ok(Self::Name),
            other => bail!(
                "unknown sort key '{other}'; expected 'rss', 'fd', 'pid', 'state', 'cpu', 'age', or 'name'"
            ),
        }
    }
}

/// State letter for sort ordering; missing/unknown sorts after all known letters (AC-006.36).
fn state_sort_letter(state_by_pid: &HashMap<u32, char>, pid: u32) -> char {
    state_by_pid.get(&pid).copied().unwrap_or(char::MAX)
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
                let cpu_a = platform::sample_pid_cpu_percent(a.agent.pid);
                let cpu_b = platform::sample_pid_cpu_percent(b.agent.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Age => {
            // Ascending age in seconds (oldest first); u64::MAX (missing data) sorts last.
            rows.sort_by(|a, b| {
                let age_a = platform::sample_pid_age_secs(a.agent.pid);
                let age_b = platform::sample_pid_age_secs(b.agent.pid);
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
                let cpu_a = platform::sample_pid_cpu_percent(a.pid);
                let cpu_b = platform::sample_pid_cpu_percent(b.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Age => {
            roots.sort_by(|a, b| {
                let age_a = platform::sample_pid_age_secs(a.pid);
                let age_b = platform::sample_pid_age_secs(b.pid);
                age_b.cmp(&age_a).then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Name => {
            roots.sort_by(|a, b| a.comm.cmp(&b.comm).then_with(|| a.pid.cmp(&b.pid)));
        }
    }
    roots
}

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
        if !comm_matches_pattern(&row.agent.comm, pattern) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.cmdline {
        let joined = cmdline_by_pid.get(&row.agent.pid).map(String::as_str).unwrap_or("");
        if !cmdline_matches_pattern(joined, pattern) {
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

fn rss_map_from_watched(watched: &[DetectedAgentWatch]) -> HashMap<u32, u64> {
    watched.iter().map(|row| (row.agent.pid, row.resource.mem_rss_bytes)).collect()
}

fn fd_map_from_watched(watched: &[DetectedAgentWatch]) -> HashMap<u32, u64> {
    watched.iter().map(|row| (row.agent.pid, row.resource.fd_count.unwrap_or(0))).collect()
}

fn apply_sort_watched(
    watched: Vec<DetectedAgentWatch>,
    sort: Option<ProcSort>,
    state_by_pid: &HashMap<u32, char>,
) -> Vec<DetectedAgentWatch> {
    match sort {
        Some(key) => sort_watched_agents(&watched, key, state_by_pid),
        None => watched,
    }
}

fn apply_sort_forests(
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
        if !comm_matches_pattern(&root.comm, pattern) {
            return false;
        }
    }
    if let Some(ref pattern) = filter.cmdline {
        let joined = cmdline_by_pid.get(&root.pid).map(String::as_str).unwrap_or("");
        if !cmdline_matches_pattern(joined, pattern) {
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
    pub pool: Option<super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::StatusJson>,
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
    pub pool: Option<super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::StatusJson>,
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
    pub pool: Option<super::PoolJson>,
    /// Proc-scan status operator panel (FR-007 / AC-007.77, proc --pid parity AC-007.86).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<super::StatusJson>,
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

fn unix_ts_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Emit one compact JSON line and flush (piped stdout is block-buffered; AC-006.18).
fn emit_ndjson_line<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    std::io::stdout().flush()?;
    Ok(())
}

fn print_host_watch_text_footer() -> Result<()> {
    print!("{}", HostResourceWatchJson::capture()?.format_text_section());
    Ok(())
}

/// Gate + host watch text companions on stderr for NDJSON watch only (AC-007.28 / AC-007.29).
/// One-shot `proc --json`, `proc --tree --json`, and `proc --pid N --json` MUST NOT call
/// this helper (AC-007.30 / AC-007.31). One-shot `proc --csv` and `proc --tree --csv` MUST NOT
/// call this helper either (AC-007.33); gate/host_watch stay in CSV companion rows on stdout
/// only (AC-007.19). One-shot text `proc`, `proc --tree`, and `proc --pid N` MUST NOT call
/// this helper either (AC-007.34); gate/host_watch stay in text sections on stdout only
/// (AC-007.17 / AC-007.20 / AC-007.21). Text watch `proc --watch` and `proc --tree --watch`
/// (no `--json`) MUST NOT call this helper either (AC-007.35); gate/host_watch and `[watch]`
/// footer stay on stdout only (inverse of AC-007.28 / AC-007.29).
fn eprint_gate_host_watch_stderr_companions(
    thermal: ThermalLevel,
    agent_count: usize,
) -> Result<()> {
    eprint!("{}", format_gate_status_section(thermal, agent_count));
    eprint!("{}", HostResourceWatchJson::capture()?.format_text_section());
    let _ = std::io::stderr().flush();
    Ok(())
}

fn append_gate_csv_companion(csv: String, gate: &sharecli_fleet::GateStatusSnapshot) -> String {
    let mut out = csv;
    out.push_str(&gate.format_csv_companion());
    out
}

fn append_host_watch_csv_companion(csv: String) -> Result<String> {
    let mut out = csv;
    out.push_str(&HostResourceWatchJson::capture()?.format_csv_companion());
    Ok(out)
}

async fn append_proc_csv_companions(
    csv: String,
    gate: &sharecli_fleet::GateStatusSnapshot,
) -> Result<String> {
    let csv = append_host_watch_csv_companion(append_gate_csv_companion(csv, gate))?;
    let (pool_json, status_json) = super::fetch_operator_pool_status_siblings().await?;
    let pool: sharecli_fleet::PoolOperatorPanel = pool_json.into();
    let status: sharecli_fleet::StatusOperatorPanel = status_json.into();
    let mut out = csv;
    out.push_str(&pool.format_csv_companion());
    out.push_str(&status.format_csv_companion());
    Ok(out)
}

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

fn state_letter_for_pid(state_by_pid: &HashMap<u32, char>, pid: u32) -> String {
    state_by_pid.get(&pid).map(|ch| ch.to_string()).unwrap_or_default()
}

fn state_json_from_char(state: char) -> String {
    if state == '?' {
        String::new()
    } else {
        state.to_string()
    }
}

fn state_text_from_detail_state(state: &str) -> String {
    if state.is_empty() {
        "-".into()
    } else {
        state.to_string()
    }
}

/// Build one JSON/CSV agent row including process state (AC-006.32).
pub fn agent_row_from_watch(
    row: &DetectedAgentWatch,
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

fn format_cmdline(cmdline: &[String]) -> String {
    if cmdline.is_empty() {
        return "(empty)".into();
    }
    cmdline.join(" ")
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

async fn render_pid_detail_once(pid: u32, json: bool, ndjson: bool) -> Result<()> {
    let scanned_agents = scan_host_agents();
    let thermal = ThermalGovernor::new().poll()?;
    let mut detail = build_proc_detail(&HostProcSource, pid)?;
    if json {
        let (pool_panel, status_panel) = super::fetch_operator_pool_status_siblings().await?;
        detail.pool = Some(pool_panel);
        detail.status = Some(status_panel);
        if ndjson {
            let line = ProcDetailNdjsonLine { ts: unix_ts_secs(), snapshot: detail };
            emit_ndjson_line(&line)?;
            eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
            return Ok(());
        }
        println!("{}", serde_json::to_string_pretty(&detail)?);
        return Ok(());
    }
    render_proc_detail_text(&detail)?;
    // AC-007.75 / AC-007.86 / AC-007.87: gate → host_watch → pool → proc-scan on stdout.
    super::print_live_pool_status_operator_sections().await?;
    Ok(())
}

async fn render_pid_detail(pid: u32, json: bool, csv: bool) -> Result<()> {
    if csv {
        let detail = build_proc_detail(&HostProcSource, pid)?;
        let body = render_proc_detail_csv(&detail);
        print!("{}", append_proc_csv_companions(body, &detail.gate).await?);
        return Ok(());
    }
    render_pid_detail_once(pid, json, false).await
}

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

/// Render one host agent inventory snapshot (text or JSON).
pub async fn render_once(
    json: bool,
    csv: bool,
    tree: bool,
    filter: &ProcFilter,
    ndjson: bool,
    sort: Option<ProcSort>,
    limit: Option<usize>,
) -> Result<()> {
    let scanned_agents = scan_host_agents();
    let thermal = ThermalGovernor::new().poll()?;
    let gate = gate_status_snapshot(thermal, scanned_agents.len());
    let watched_all = watch_detected_agents(&scanned_agents);
    let rss_by_pid = rss_map_from_watched(&watched_all);
    let fd_by_pid = fd_map_from_watched(&watched_all);
    let agent_pids: Vec<u32> = scanned_agents.iter().map(|a| a.pid).collect();
    let ppid_by_pid = build_agent_ppid_map(&HostProcSource, &agent_pids);
    let cmdline_by_pid = build_agent_cmdline_map(&HostProcSource, &agent_pids);
    let state_by_pid = build_agent_state_map(&HostProcSource, &agent_pids);

    if tree {
        let forests = filter_agent_forests(
            &build_host_agent_forests(),
            filter,
            &rss_by_pid,
            &fd_by_pid,
            &cmdline_by_pid,
            &state_by_pid,
        );
        let forests = apply_sort_forests(forests, sort, &rss_by_pid, &fd_by_pid, &state_by_pid);
        let forests = limit_agent_forests(forests, limit);
        let tree_state_by_pid = build_forest_state_map(&HostProcSource, &forests);
        // One-shot `proc --tree --csv` MUST NOT print gate/host_watch stderr companions (AC-007.33).
        if csv {
            print!(
                "{}",
                append_proc_csv_companions(
                    render_agent_tree_csv(&forests, &rss_by_pid, &fd_by_pid, &tree_state_by_pid,),
                    &gate,
                )
                .await?
            );
            return Ok(());
        }
        let snap = AgentTreeSnapshot {
            forests: forests
                .iter()
                .map(|root| agent_tree_node_to_json(root, &tree_state_by_pid))
                .collect(),
            roots: forests.len(),
            gate,
            host_watch: HostResourceWatchJson::capture()?,
            pool: None,
            status: None,
        };
        if json {
            let (pool_panel, status_panel) = super::fetch_operator_pool_status_siblings().await?;
            let snap =
                AgentTreeSnapshot { pool: Some(pool_panel), status: Some(status_panel), ..snap };
            if ndjson {
                let line = AgentTreeNdjsonLine { ts: unix_ts_secs(), snapshot: snap };
                emit_ndjson_line(&line)?;
                eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&snap)?);
            return Ok(());
        }
        // One-shot `proc --tree` text MUST NOT print gate/host_watch stderr companions (AC-007.34).
        render_agent_tree(&forests, &tree_state_by_pid);
        print!("{}", format_gate_status_section(thermal, scanned_agents.len()));
        print_host_watch_text_footer()?;
        // AC-007.34 / AC-007.75: gate → host_watch → pool → proc-scan on stdout after tree body.
        super::print_live_pool_status_operator_sections().await?;
        return Ok(());
    }

    let watched = limit_watched_agents(
        apply_sort_watched(
            filter_watched_agents(
                &watched_all,
                filter,
                &ppid_by_pid,
                &cmdline_by_pid,
                &state_by_pid,
            ),
            sort,
            &state_by_pid,
        ),
        limit,
    );
    // One-shot `proc --csv` MUST NOT print gate/host_watch stderr companions (AC-007.33).
    if csv {
        print!(
            "{}",
            append_proc_csv_companions(render_agent_inventory_csv(&watched, &state_by_pid), &gate,)
                .await?
        );
        return Ok(());
    }
    if json {
        let (pool_panel, status_panel) = super::fetch_operator_pool_status_siblings().await?;
        let snap = AgentProcSnapshot {
            agents: watched.iter().map(|row| agent_row_from_watch(row, &state_by_pid)).collect(),
            scanned: scanned_agents.len(),
            watched: watched.len(),
            gate,
            host_watch: HostResourceWatchJson::capture()?,
            pool: Some(pool_panel),
            status: Some(status_panel),
        };
        if ndjson {
            let line = AgentProcNdjsonLine { ts: unix_ts_secs(), snapshot: snap };
            emit_ndjson_line(&line)?;
            eprint_gate_host_watch_stderr_companions(thermal, scanned_agents.len())?;
            return Ok(());
        }
        println!("{}", serde_json::to_string_pretty(&snap)?);
        return Ok(());
    }
    // One-shot `proc` text MUST NOT print gate/host_watch stderr companions (AC-007.34).
    render_agent_inventory(&watched, scanned_agents.len(), &state_by_pid);
    print!("{}", format_gate_status_section(thermal, scanned_agents.len()));
    print_host_watch_text_footer()?;
    // AC-007.34 / AC-007.75: gate → host_watch → pool → proc-scan on stdout after inventory body.
    super::print_live_pool_status_operator_sections().await?;
    Ok(())
}

/// `sharecli proc` — list host-detected agents with live RSS/FD samples.
/// Reject inventory-mode flags when `--pid` selects detail mode (AC-007.92).
// CLI flag aggregation: one Option per inventory filter keeps the dispatch
// layer mechanical; the wide signature is intentional.
#[allow(clippy::too_many_arguments)]
fn reject_pid_inventory_combos(
    tree: bool,
    family: &Option<String>,
    exclude_family: &Option<String>,
    comm: &Option<String>,
    cmdline: &Option<String>,
    state: &Option<String>,
    min_rss: &Option<String>,
    max_rss: &Option<String>,
    min_fd: &Option<String>,
    max_fd: &Option<String>,
    sort: &Option<String>,
    limit: Option<u64>,
) -> Result<()> {
    let mut conflicts: Vec<&str> = Vec::new();
    if tree {
        conflicts.push("--tree");
    }
    if family.is_some() {
        conflicts.push("--family");
    }
    if exclude_family.is_some() {
        conflicts.push("--exclude-family");
    }
    if comm.is_some() {
        conflicts.push("--comm");
    }
    if cmdline.is_some() {
        conflicts.push("--cmdline");
    }
    if state.is_some() {
        conflicts.push("--state");
    }
    if min_rss.is_some() {
        conflicts.push("--min-rss");
    }
    if max_rss.is_some() {
        conflicts.push("--max-rss");
    }
    if min_fd.is_some() {
        conflicts.push("--min-fd");
    }
    if max_fd.is_some() {
        conflicts.push("--max-fd");
    }
    if sort.is_some() {
        conflicts.push("--sort");
    }
    if limit.is_some() {
        conflicts.push("--limit");
    }
    if conflicts.is_empty() {
        return Ok(());
    }
    bail!(
        "--pid cannot be combined with {} (AC-007.92); use inventory mode without --pid",
        conflicts.join(", ")
    )
}

// CLI entry point: all `proc` flags are threaded through as individual
// arguments by the shared dispatch layer; the wide signature is intentional.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    json: bool,
    csv: bool,
    tree: bool,
    watch: Option<u64>,
    family: Option<String>,
    exclude_family: Option<String>,
    comm: Option<String>,
    cmdline: Option<String>,
    state: Option<String>,
    min_rss: Option<String>,
    max_rss: Option<String>,
    min_fd: Option<String>,
    max_fd: Option<String>,
    sort: Option<String>,
    limit: Option<u64>,
    pid: Option<u32>,
    ppid: Option<u32>,
) -> Result<()> {
    if csv && json {
        bail!("--csv cannot be combined with --json");
    }
    if pid.is_some() && ppid.is_some() {
        bail!("--ppid cannot be combined with --pid");
    }
    if pid.is_some() {
        // AC-007.92: --pid is detail mode; inventory filters/tree MUST fail loudly.
        reject_pid_inventory_combos(
            tree,
            &family,
            &exclude_family,
            &comm,
            &cmdline,
            &state,
            &min_rss,
            &max_rss,
            &min_fd,
            &max_fd,
            &sort,
            limit,
        )?;
    }
    if let Some(target_pid) = pid {
        if let Some(interval_secs) = watch {
            if interval_secs == 0 {
                bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            let period = Duration::from_secs(interval_secs);
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    // AC-007.91: frame marker + full PID CSV body + `# [watch]` on stdout.
                    println!("{PROC_PID_CSV_WATCH_FRAME_MARKER}");
                    render_pid_detail(target_pid, false, true).await?;
                } else {
                    render_pid_detail_once(target_pid, json, ndjson).await?;
                }
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                if ndjson {
                    let footer = format!(
                        "\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop."
                    );
                    eprint!("{footer}");
                    let _ = std::io::stderr().flush();
                } else if csv_watch {
                    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.94: flush so `# [watch]` reaches pipe consumers this tick.
                    std::io::stdout().flush()?;
                } else {
                    println!("\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.96: flush text `[watch]` footer same tick (parity with CSV).
                    std::io::stdout().flush()?;
                }
                let idle = period.saturating_sub(cycle_start.elapsed());
                tokio::select! {
                    _ = sleep(idle) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    }
                }
            }
            return Ok(());
        }
        return render_pid_detail(target_pid, json, csv).await;
    }
    let filter = ProcFilter::from_cli(
        family,
        exclude_family,
        comm,
        cmdline,
        state,
        min_rss,
        max_rss,
        min_fd,
        max_fd,
        ppid,
    )?;
    let sort_key = ProcSort::from_cli(sort.as_deref())?;
    let row_limit = parse_proc_limit(limit)?;
    match watch {
        None => render_once(json, csv, tree, &filter, false, sort_key, row_limit).await,
        Some(interval_secs) => {
            if interval_secs == 0 {
                bail!("--watch interval must be >= 1 second");
            }
            let ndjson = json;
            let csv_watch = csv;
            let period = Duration::from_secs(interval_secs);
            loop {
                let cycle_start = std::time::Instant::now();
                if !ndjson && !csv_watch {
                    print!("\x1b[2J\x1b[H");
                }
                if csv_watch {
                    println!("{PROC_CSV_WATCH_FRAME_MARKER}");
                }
                render_once(json, csv, tree, &filter, ndjson, sort_key, row_limit).await?;
                if !ndjson {
                    std::io::stdout().flush()?;
                }
                // Text watch: gate/host_watch + `[watch]` footer on stdout only (AC-007.35).
                // CSV watch: frame marker + full CSV body + `# [watch]` comment on stdout (AC-007.88).
                // NDJSON watch: companions + footer on stderr (AC-007.28 / AC-007.29).
                let footer =
                    format!("\n[watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                if ndjson {
                    eprint!("{footer}");
                    let _ = std::io::stderr().flush();
                } else if csv_watch {
                    println!("# [watch] Refreshing every {interval_secs}s — press Ctrl-C to stop.");
                    // AC-007.94: flush so `# [watch]` reaches pipe consumers this tick.
                    std::io::stdout().flush()?;
                } else {
                    println!("{footer}");
                    // AC-007.96: flush text `[watch]` footer same tick (parity with CSV).
                    std::io::stdout().flush()?;
                }
                let idle = period.saturating_sub(cycle_start.elapsed());
                tokio::select! {
                    _ = sleep(idle) => {},
                    _ = tokio::signal::ctrl_c() => {
                        if ndjson {
                            eprintln!("\nExiting watch mode.");
                        } else {
                            println!("\nExiting watch mode.");
                        }
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}

mod platform;
#[cfg(test)]
mod tests;
