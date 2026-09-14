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

/// Read `/proc/{pid}/stat` and return `(utime + stime, starttime)` in clock ticks.
/// Returns `None` when the process is gone or `/proc` is unavailable.
#[cfg(target_os = "linux")]
fn read_pid_stat_linux(pid: u32) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // /proc/{pid}/stat layout: `pid (comm) state ppid pgrp session tty_nr tpgid flags
    //   minflt cminflt majflt cmajflt utime stime cutime cstime priority nice num_threads
    //   itrealvalue starttime ...`. The COMM field can contain spaces, so we MUST split
    //   on the last `)` before tokenizing the trailing fields.
    let rparen = text.rfind(')')?;
    let rest = &text[rparen + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After ')', the field indices (0-based here) are: 0=state 1=ppid 2=pgrp 3=session
    // 4=tty_nr 5=tpgid 6=flags 7=minflt 8=cminflt 9=majflt 10=cmajflt 11=utime 12=stime
    // 13=cutime 14=cstime ... 19=starttime.
    if fields.len() < 20 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let starttime: u64 = fields[19].parse().ok()?;
    Some((utime + stime, starttime))
}

#[cfg(not(target_os = "linux"))]
fn read_pid_stat_linux(_pid: u32) -> Option<(u64, u64)> {
    None
}

/// Read host uptime in seconds from `/proc/uptime` (first field).
#[cfg(target_os = "linux")]
fn read_uptime_secs() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    text.split_whitespace().next()?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn read_uptime_secs() -> Option<f64> {
    None
}

/// `clock ticks per second` for `/proc/stat` and `/proc/{pid}/stat`. Linux default is 100.
#[cfg(target_os = "linux")]
const CLK_TCK: u64 = 100;

#[cfg(not(target_os = "linux"))]
const CLK_TCK: u64 = 100;

/// Sample one-shot CPU usage percent for a PID using `/proc/{pid}/stat` + `/proc/uptime`.
/// Returns `0.0` when the data is unavailable (non-Linux or process gone).
fn sample_pid_cpu_percent(pid: u32) -> f64 {
    let Some((cpu_ticks, starttime)) = read_pid_stat_linux(pid) else {
        return 0.0;
    };
    let Some(uptime_secs) = read_uptime_secs() else {
        return 0.0;
    };
    let tck = CLK_TCK as f64;
    let starttime_secs = starttime as f64 / tck;
    let elapsed = uptime_secs - starttime_secs;
    if elapsed <= 0.0 {
        return 0.0;
    }
    let cpu_secs = cpu_ticks as f64 / tck;
    (cpu_secs / elapsed) * 100.0
}

/// Sample one-shot process age in seconds for a PID. Returns `u64::MAX` when data is
/// unavailable (non-Linux or process gone) so missing rows sort last under `Age`.
fn sample_pid_age_secs(pid: u32) -> u64 {
    let Some((_, starttime)) = read_pid_stat_linux(pid) else {
        return u64::MAX;
    };
    let Some(uptime_secs) = read_uptime_secs() else {
        return u64::MAX;
    };
    let starttime_secs = starttime as f64 / CLK_TCK as f64;
    let age = uptime_secs - starttime_secs;
    if age < 0.0 {
        0
    } else {
        age as u64
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
                let cpu_a = sample_pid_cpu_percent(a.agent.pid);
                let cpu_b = sample_pid_cpu_percent(b.agent.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.agent.pid.cmp(&b.agent.pid))
            });
        }
        ProcSort::Age => {
            // Ascending age in seconds (oldest first); u64::MAX (missing data) sorts last.
            rows.sort_by(|a, b| {
                let age_a = sample_pid_age_secs(a.agent.pid);
                let age_b = sample_pid_age_secs(b.agent.pid);
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
                let cpu_a = sample_pid_cpu_percent(a.pid);
                let cpu_b = sample_pid_cpu_percent(b.pid);
                cpu_b
                    .partial_cmp(&cpu_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
        ProcSort::Age => {
            roots.sort_by(|a, b| {
                let age_a = sample_pid_age_secs(a.pid);
                let age_b = sample_pid_age_secs(b.pid);
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

/// Host inventory from a proc source (used by `ps --all` tests).
#[cfg(test)]
fn host_agent_inventory_from_source(
    source: &dyn sharecli_fleet::ProcSource,
) -> (Vec<DetectedAgentWatch>, usize) {
    let agents = sharecli_fleet::scan_agents(source);
    let watched = watch_detected_agents(&agents);
    (watched, agents.len())
}

#[cfg(test)]
mod tests {
    use sharecli_fleet::collect_forest_pids;
    use sharecli_fleet::proc_scan::{DetectedAgent, FakeProcSource, ProcSnapshot};

    use super::*;

    #[test]
    fn agent_row_from_watch_formats_rss() {
        let row = agent_row_from_watch(
            &DetectedAgentWatch {
                agent: DetectedAgent { pid: 42, family: "claude", comm: "claude".into() },
                resource: sharecli_fleet::AgentResourceSample {
                    mem_rss_bytes: 52_428_800,
                    fd_count: Some(10),
                },
            },
            &HashMap::from([(42, 'R')]),
        );
        assert_eq!(row.mem_rss, "50M");
        assert_eq!(row.fd_count, Some(10));
        assert_eq!(row.state, "R");
    }

    #[test]
    fn host_inventory_from_fixture() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        }]);
        let (watched, scanned) = host_agent_inventory_from_source(&src);
        assert_eq!(scanned, 1);
        assert_eq!(watched.len(), 0, "fixture PID is not live on host");
        let _ = watched;
    }

    #[test]
    fn zero_watch_interval_is_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(super::run(
                false,
                false,
                false,
                Some(0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ))
            .expect_err("watch 0 MUST fail");
        assert!(
            err.to_string().contains(">= 1"),
            "error MUST mention minimum interval; got: {err}"
        );
    }

    #[test]
    fn ndjson_line_includes_ts_and_agents() {
        let line = AgentProcNdjsonLine {
            ts: 1_750_000_000,
            snapshot: AgentProcSnapshot {
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: sharecli_fleet::GateStatusSnapshot {
                    thermal_pressure: "GREEN".into(),
                    detected_agents: 0,
                    agent_total_rss_bytes: 0,
                    agent_contention: "OK".into(),
                    gate_decision: "ADMIT".into(),
                },
                host_watch: HostResourceWatchJson::default(),
                pool: None,
                status: None,
            },
        };
        let json = serde_json::to_string(&line).expect("serialize");
        assert!(json.contains("\"ts\":1750000000"));
        assert!(json.contains("\"agents\":[]"));
        assert!(json.contains("\"host_watch\""));
    }

    #[test]
    fn ndjson_line_agent_rows_include_state() {
        let line = AgentProcNdjsonLine {
            ts: 1_750_000_000,
            snapshot: AgentProcSnapshot {
                agents: vec![AgentProcRow {
                    pid: 42,
                    family: "claude".into(),
                    comm: "claude".into(),
                    state: "R".into(),
                    mem_rss_bytes: 100,
                    mem_rss: "100B".into(),
                    fd_count: None,
                }],
                scanned: 1,
                watched: 1,
                gate: sharecli_fleet::GateStatusSnapshot {
                    thermal_pressure: "GREEN".into(),
                    detected_agents: 1,
                    agent_total_rss_bytes: 100,
                    agent_contention: "OK".into(),
                    gate_decision: "ADMIT".into(),
                },
                host_watch: HostResourceWatchJson::default(),
                pool: None,
                status: None,
            },
        };
        let json = serde_json::to_string(&line).expect("serialize");
        assert!(
            json.contains("\"state\":\"R\""),
            "NDJSON watch line MUST include agent state (AC-006.37); got: {json}"
        );
    }

    #[test]
    fn build_forest_state_map_includes_child_pids() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 50,
                ppid: 1,
                comm: "claude".into(),
                cmdline: vec!["claude".into()],
                state: 'S',
            },
            ProcSnapshot {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                cmdline: vec!["node".into()],
                state: 'R',
            },
        ]);
        let forests = sharecli_fleet::build_agent_forests(&src);
        assert_eq!(collect_forest_pids(&forests), vec![50, 51]);
        let map = build_forest_state_map(&src, &forests);
        assert_eq!(map.get(&51), Some(&'R'));
    }

    #[test]
    fn tree_json_from_fixture() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 50,
                ppid: 1,
                comm: "cursor-agent".into(),
                cmdline: vec!["cursor-agent".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                cmdline: vec!["node".into()],
                state: 'R',
            },
        ]);
        let forests = sharecli_fleet::build_agent_forests(&src);
        let state_by_pid = HashMap::from([(50, 'R'), (51, 'R')]);
        let snap = AgentTreeSnapshot {
            forests: forests
                .iter()
                .map(|root| agent_tree_node_to_json(root, &state_by_pid))
                .collect(),
            roots: forests.len(),
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        assert_eq!(snap.roots, 1);
        assert_eq!(snap.forests[0].state, "R");
        assert_eq!(snap.forests[0].children.len(), 1);
        assert_eq!(snap.forests[0].children[0].pid, 51);
        assert_eq!(snap.forests[0].children[0].state, "R");
    }

    #[test]
    fn build_proc_detail_missing_pid_fails() {
        let src = FakeProcSource::new(vec![]);
        let err = build_proc_detail(&src, 42).expect_err("missing pid");
        assert!(
            err.to_string().contains("not found"),
            "error MUST mention missing process; got: {err}"
        );
    }

    #[test]
    fn pid_csv_watch_zero_interval_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(super::run(
                false,
                true,
                false,
                Some(0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(42),
                None,
            ))
            .expect_err("pid+csv+watch 0 MUST fail");
        assert!(
            err.to_string().contains("--watch") || err.to_string().contains(">= 1"),
            "error MUST mention watch interval; got: {err}"
        );
    }

    #[test]
    fn pid_tree_combo_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(super::run(
                false,
                false,
                true,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(42),
                None,
            ))
            .expect_err("pid+tree MUST fail");
        assert!(
            err.to_string().contains("--tree") && err.to_string().contains("AC-007.92"),
            "error MUST mention --tree / AC-007.92; got: {err}"
        );
    }

    #[test]
    fn pid_family_sort_limit_combo_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(super::run(
                false,
                false,
                false,
                None,
                Some("claude".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("rss".into()),
                Some(5),
                Some(42),
                None,
            ))
            .expect_err("pid+family+sort+limit MUST fail");
        let msg = err.to_string();
        assert!(msg.contains("--family"), "got: {msg}");
        assert!(msg.contains("--sort"), "got: {msg}");
        assert!(msg.contains("--limit"), "got: {msg}");
        assert!(msg.contains("AC-007.92"), "got: {msg}");
    }

    fn fixture_row(pid: u32, family: &'static str, comm: &str, rss: u64) -> DetectedAgentWatch {
        DetectedAgentWatch {
            agent: DetectedAgent { pid, family, comm: comm.into() },
            resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: Some(0) },
        }
    }

    fn fixture_inventory() -> Vec<DetectedAgentWatch> {
        vec![
            fixture_row(100, "claude", "claude", 50_000_000),
            fixture_row(50, "claude", "claude", 200_000_000),
            fixture_row(75, "claude", "claude", 100_000_000),
            fixture_row(25, "claude", "claude", 75_000_000),
            fixture_row(150, "claude", "claude", 300_000_000),
            fixture_row(10, "claude", "claude", 10_000_000),
        ]
    }

    #[test]
    fn sort_rss_desc_then_limit_caps_rows() {
        // AC-006.41 / AC-006.21: --sort rss --limit 5 returns at most 5 rows, RSS descending.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Rss, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(5));
        assert!(limited.len() <= 5, "--limit 5 MUST cap at 5 rows; got {}", limited.len());
        // Confirm strict descending RSS order across the limited slice.
        let rss_seq: Vec<u64> = limited.iter().map(|r| r.resource.mem_rss_bytes).collect();
        let mut prev = u64::MAX;
        for rss in &rss_seq {
            assert!(*rss <= prev, "RSS MUST be descending; saw {rss} after {prev}");
            prev = *rss;
        }
        // First row MUST be the largest RSS fixture (pid 150 = 300M).
        assert_eq!(limited[0].agent.pid, 150);
    }

    #[test]
    fn sort_pid_asc_then_limit_caps_rows() {
        // AC-006.19 / AC-006.21: --sort pid --limit 3 returns at most 3 rows, PID ascending.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Pid, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(3));
        assert!(limited.len() <= 3, "--limit 3 MUST cap at 3 rows; got {}", limited.len());
        let pid_seq: Vec<u32> = limited.iter().map(|r| r.agent.pid).collect();
        let mut prev = 0u32;
        for pid in &pid_seq {
            assert!(*pid > prev, "PID MUST be ascending; saw {pid} after {prev}");
            prev = *pid;
        }
        assert_eq!(limited[0].agent.pid, 10);
        assert_eq!(limited[1].agent.pid, 25);
        assert_eq!(limited[2].agent.pid, 50);
    }

    #[test]
    fn sort_name_ascending_alphabetical() {
        // AC-006.41: --sort name sorts by COMM alphabetical; PID tie-break ascending.
        let inventory = vec![
            fixture_row(3, "claude", "zsh", 1),
            fixture_row(1, "claude", "bash", 1),
            fixture_row(2, "claude", "alpha", 1),
        ];
        let sorted = sort_watched_agents(&inventory, ProcSort::Name, &HashMap::new());
        let comms: Vec<&str> = sorted.iter().map(|r| r.agent.comm.as_str()).collect();
        assert_eq!(comms, vec!["alpha", "bash", "zsh"]);
    }

    #[test]
    fn sort_name_pid_tie_break_ascending() {
        // Same COMM, different PID → PID tie-break ascending.
        let inventory = vec![
            fixture_row(30, "claude", "claude", 1),
            fixture_row(10, "claude", "claude", 1),
            fixture_row(20, "claude", "claude", 1),
        ];
        let sorted = sort_watched_agents(&inventory, ProcSort::Name, &HashMap::new());
        let pids: Vec<u32> = sorted.iter().map(|r| r.agent.pid).collect();
        assert_eq!(pids, vec![10, 20, 30]);
    }

    #[test]
    fn proc_sort_parses_cpu_age_name() {
        // AC-006.41: --sort accepts cpu, age, name keys alongside the historical set.
        assert_eq!("cpu".parse::<ProcSort>().unwrap(), ProcSort::Cpu);
        assert_eq!("age".parse::<ProcSort>().unwrap(), ProcSort::Age);
        assert_eq!("name".parse::<ProcSort>().unwrap(), ProcSort::Name);
        assert_eq!("NAME".parse::<ProcSort>().unwrap(), ProcSort::Name);
    }

    #[test]
    fn proc_sort_unknown_key_lists_new_options() {
        // AC-006.41: error message MUST enumerate cpu/age/name as accepted sort keys.
        let err = "bogus".parse::<ProcSort>().expect_err("unknown sort key MUST fail");
        let msg = err.to_string();
        assert!(msg.contains("cpu"), "error MUST list 'cpu'; got: {msg}");
        assert!(msg.contains("age"), "error MUST list 'age'; got: {msg}");
        assert!(msg.contains("name"), "error MUST list 'name'; got: {msg}");
    }

    #[test]
    fn parse_proc_limit_zero_is_rejected() {
        // AC-006.21: --limit 0 MUST be rejected as invalid (>= 1).
        let err = parse_proc_limit(Some(0)).expect_err("--limit 0 MUST fail");
        assert!(
            err.to_string().contains(">= 1") || err.to_string().contains("must be"),
            "error MUST mention minimum; got: {err}"
        );
    }

    #[test]
    fn limit_under_inventory_size_returns_inventory() {
        // AC-006.21: --limit N larger than inventory returns the full slice, capped order intact.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Pid, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(50));
        assert_eq!(limited.len(), inventory.len(), "limit MUST NOT pad below inventory size");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: comm_matches_pattern / cmdline_matches_pattern
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn comm_matches_pattern_case_insensitive() {
        assert!(comm_matches_pattern("Claude", "claude"));
        assert!(comm_matches_pattern("CLAUDE", "claude"));
        assert!(comm_matches_pattern("claude", "CLAUDE"));
    }

    #[test]
    fn comm_matches_pattern_no_match() {
        assert!(!comm_matches_pattern("node", "claude"));
        assert!(!comm_matches_pattern("", "claude"));
    }

    #[test]
    fn cmdline_matches_pattern_case_insensitive() {
        assert!(cmdline_matches_pattern("/usr/bin/Claude --help", "claude"));
        assert!(cmdline_matches_pattern("claude serve", "SERVE"));
    }

    #[test]
    fn cmdline_matches_pattern_empty_string() {
        assert!(!cmdline_matches_pattern("", "anything"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_proc_state edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_proc_state_all_valid_uppercase() {
        for ch in ['R', 'S', 'D', 'Z', 'T', 'X', 'K', 'W', 'P', 'I'] {
            let result = parse_proc_state(&ch.to_string()).expect("valid state MUST parse");
            assert_eq!(result, ch, "uppercase {ch} must normalize to itself");
        }
    }

    #[test]
    fn parse_proc_state_lowercase_normalizes_to_uppercase() {
        for (input, expected) in [
            ('r', 'R'),
            ('s', 'S'),
            ('d', 'D'),
            ('z', 'Z'),
            ('k', 'K'),
            ('w', 'W'),
            ('p', 'P'),
            ('i', 'I'),
        ] {
            let result = parse_proc_state(&input.to_string()).expect("valid lowercase MUST parse");
            assert_eq!(result, expected, "lowercase '{input}' must normalize to '{expected}'");
        }
    }

    #[test]
    fn parse_proc_state_lowercase_t_and_x_preserved() {
        assert_eq!(parse_proc_state("t".into()).unwrap(), 't');
        assert_eq!(parse_proc_state("x".into()).unwrap(), 'x');
    }

    #[test]
    fn parse_proc_state_empty_rejected() {
        parse_proc_state("").expect_err("empty MUST fail");
        parse_proc_state("  ").expect_err("whitespace-only MUST fail");
    }

    #[test]
    fn parse_proc_state_multi_char_rejected() {
        parse_proc_state("RS").expect_err("multi-char MUST fail");
        parse_proc_state("ab").expect_err("multi-char MUST fail");
    }

    #[test]
    fn parse_proc_state_invalid_char_rejected() {
        let err = parse_proc_state("Q").expect_err("invalid char MUST fail");
        assert!(err.to_string().contains("Q"), "error MUST mention the bad char; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_fd_count
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_fd_count_valid_number() {
        assert_eq!(parse_fd_count("0", "--min-fd").unwrap(), 0);
        assert_eq!(parse_fd_count("42", "--max-fd").unwrap(), 42);
        assert_eq!(parse_fd_count("1024", "--min-fd").unwrap(), 1024);
    }

    #[test]
    fn parse_fd_count_invalid_string_rejected() {
        let err = parse_fd_count("abc", "--min-fd").expect_err("non-numeric MUST fail");
        assert!(err.to_string().contains("abc"), "error MUST mention the bad value; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: csv_escape_field
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn csv_escape_field_no_escaping_needed() {
        assert_eq!(csv_escape_field("hello"), "hello");
        assert_eq!(csv_escape_field(""), "");
    }

    #[test]
    fn csv_escape_field_comma_wrapped_in_quotes() {
        assert_eq!(csv_escape_field("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_escape_field_double_quote_escaped_and_wrapped() {
        assert_eq!(csv_escape_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_field_newline_wrapped_in_quotes() {
        assert_eq!(csv_escape_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn csv_escape_field_carriage_return_wrapped() {
        assert_eq!(csv_escape_field("a\rb"), "\"a\rb\"");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: format_cmdline
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn format_cmdline_empty_returns_placeholder() {
        assert_eq!(format_cmdline(&[]), "(empty)");
    }

    #[test]
    fn format_cmdline_single_element() {
        assert_eq!(format_cmdline(&["claude".into()]), "claude");
    }

    #[test]
    fn format_cmdline_multiple_elements_joined() {
        let args = vec!["claude".into(), "serve".into(), "--port".into(), "8080".into()];
        assert_eq!(format_cmdline(&args), "claude serve --port 8080");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: state_letter_for_pid / state_json_from_char / state_text_from_detail_state
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn state_letter_for_pid_present() {
        let map = HashMap::from([(42, 'R'), (99, 'S')]);
        assert_eq!(state_letter_for_pid(&map, 42), "R");
        assert_eq!(state_letter_for_pid(&map, 99), "S");
    }

    #[test]
    fn state_letter_for_pid_missing_returns_empty() {
        let map = HashMap::from([(42, 'R')]);
        assert_eq!(state_letter_for_pid(&map, 999), "");
    }

    #[test]
    fn state_json_from_char_normal() {
        assert_eq!(state_json_from_char('R'), "R");
        assert_eq!(state_json_from_char('S'), "S");
        assert_eq!(state_json_from_char('Z'), "Z");
    }

    #[test]
    fn state_json_from_char_question_returns_empty() {
        assert_eq!(state_json_from_char('?'), "");
    }

    #[test]
    fn state_text_from_detail_state_empty_returns_dash() {
        assert_eq!(state_text_from_detail_state(""), "-");
    }

    #[test]
    fn state_text_from_detail_state_nonempty_returns_self() {
        assert_eq!(state_text_from_detail_state("R"), "R");
        assert_eq!(state_text_from_detail_state("S"), "S");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_proc_detail_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_proc_detail_csv_includes_header_and_body() {
        let detail = ProcDetailSnapshot {
            pid: 1234,
            ppid: 1,
            parent_comm: Some("init".into()),
            comm: "claude".into(),
            state: "R".into(),
            cmdline: vec!["claude".into()],
            family: Some("claude".into()),
            agent_ancestor: None,
            mem_rss_bytes: 52_428_800,
            mem_rss: "50M".into(),
            fd_count: Some(12),
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 1,
                agent_total_rss_bytes: 52_428_800,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        let csv = render_proc_detail_csv(&detail);
        assert!(csv.starts_with("pid,ppid,comm,state,mem_rss_bytes,mem_rss,fd_count\n"));
        assert!(csv.contains("1234,1,claude,R,52428800,50M,12\n"));
    }

    #[test]
    fn render_proc_detail_csv_empty_fd_count() {
        let detail = ProcDetailSnapshot {
            pid: 100,
            ppid: 1,
            parent_comm: None,
            comm: "node".into(),
            state: "S".into(),
            cmdline: vec![],
            family: None,
            agent_ancestor: None,
            mem_rss_bytes: 0,
            mem_rss: "0B".into(),
            fd_count: None,
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        let csv = render_proc_detail_csv(&detail);
        // fd_count column should be empty when None
        assert!(
            csv.contains(",\n") || csv.ends_with(",\n"),
            "missing fd MUST produce empty field; got: {csv}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: append_gate_csv_companion
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn append_gate_csv_companion_appends_to_existing() {
        let gate = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 3,
            agent_total_rss_bytes: 1_000_000,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let base = "header\nrow1\n".to_string();
        let result = append_gate_csv_companion(base.clone(), &gate);
        assert!(result.starts_with(&base), "base content MUST be preserved");
        assert!(result.len() > base.len(), "companion MUST append content");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_agent_inventory_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_agent_inventory_csv_header_and_rows() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "cursor", "cursor-agent", 2_000_000),
        ];
        let state_by_pid = HashMap::from([(10, 'R'), (20, 'S')]);
        let csv = render_agent_inventory_csv(&inventory, &state_by_pid);
        assert!(csv.starts_with("pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"));
        assert!(csv.contains("10,claude,claude,R,1000000,"));
        assert!(csv.contains("20,cursor,cursor-agent,S,2000000,"));
    }

    #[test]
    fn render_agent_inventory_csv_empty_inventory() {
        let csv = render_agent_inventory_csv(&[], &HashMap::new());
        assert!(csv.starts_with("pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"));
        // Empty inventory still has header + trailing newline
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "empty inventory MUST have only header; got {} lines",
            lines.len()
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: filter_watched_agents
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn filter_watched_agents_family_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "cursor", "cursor-agent", 2_000_000),
        ];
        let filter = ProcFilter { family: Some("claude".into()), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 10);
    }

    #[test]
    fn filter_watched_agents_comm_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "claude", "node", 2_000_000),
        ];
        let filter = ProcFilter { comm: Some("node".into()), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 20);
    }

    #[test]
    fn filter_watched_agents_empty_filter_returns_all() {
        let inventory = fixture_inventory();
        let filter = ProcFilter::default();
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), inventory.len());
    }

    #[test]
    fn filter_watched_agents_rss_bounds() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 500),
            fixture_row(20, "claude", "claude", 1500),
            fixture_row(30, "claude", "claude", 2500),
        ];
        let filter = ProcFilter {
            min_rss_bytes: Some(1000),
            max_rss_bytes: Some(2000),
            ..Default::default()
        };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 20);
    }

    #[test]
    fn filter_watched_agents_state_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "claude", "claude", 2_000_000),
        ];
        let state_by_pid = HashMap::from([(10, 'R'), (20, 'S')]);
        let filter = ProcFilter { state: Some('R'), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &state_by_pid,
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 10);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: limit_agent_forests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn limit_agent_forests_none_returns_all() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
        ];
        let result = limit_agent_forests(forests.clone(), None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn limit_agent_forests_some_caps() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 30, ppid: 1, comm: "c".into(), family: None, children: vec![] },
        ];
        let result = limit_agent_forests(forests, Some(2));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].pid, 10);
        assert_eq!(result[1].pid, 20);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: sort_agent_forests by name
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn sort_agent_forests_by_name_ascending() {
        let forests = vec![
            AgentTreeNode { pid: 3, ppid: 1, comm: "zsh".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 1, ppid: 1, comm: "alpha".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 2, ppid: 1, comm: "beta".into(), family: None, children: vec![] },
        ];
        let result = sort_agent_forests(
            &forests,
            ProcSort::Name,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        let comms: Vec<&str> = result.iter().map(|n| n.comm.as_str()).collect();
        assert_eq!(comms, vec!["alpha", "beta", "zsh"]);
    }

    #[test]
    fn sort_agent_forests_by_rss_desc() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
        ];
        let rss_by_pid = HashMap::from([(10, 500), (20, 1000)]);
        let result = sort_agent_forests(
            &forests,
            ProcSort::Rss,
            &rss_by_pid,
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(result[0].pid, 20, "higher RSS MUST sort first");
        assert_eq!(result[1].pid, 10);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcFilter::active
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_filter_active_empty_is_false() {
        assert!(!ProcFilter::default().active());
    }

    #[test]
    fn proc_filter_active_with_family_is_true() {
        let f = ProcFilter { family: Some("claude".into()), ..Default::default() };
        assert!(f.active());
    }

    #[test]
    fn proc_filter_active_with_state_is_true() {
        let f = ProcFilter { state: Some('R'), ..Default::default() };
        assert!(f.active());
    }

    #[test]
    fn proc_filter_active_with_rss_bounds_is_true() {
        let f = ProcFilter { min_rss_bytes: Some(1000), ..Default::default() };
        assert!(f.active());
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcSort::from_cli
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_sort_from_cli_none_returns_none() {
        assert!(ProcSort::from_cli(None).unwrap().is_none());
    }

    #[test]
    fn proc_sort_from_cli_valid_key() {
        assert_eq!(ProcSort::from_cli(Some("rss")).unwrap(), Some(ProcSort::Rss));
        assert_eq!(ProcSort::from_cli(Some("fd")).unwrap(), Some(ProcSort::Fd));
        assert_eq!(ProcSort::from_cli(Some("pid")).unwrap(), Some(ProcSort::Pid));
        assert_eq!(ProcSort::from_cli(Some("state")).unwrap(), Some(ProcSort::State));
        assert_eq!(ProcSort::from_cli(Some("cpu")).unwrap(), Some(ProcSort::Cpu));
        assert_eq!(ProcSort::from_cli(Some("age")).unwrap(), Some(ProcSort::Age));
        assert_eq!(ProcSort::from_cli(Some("name")).unwrap(), Some(ProcSort::Name));
    }

    #[test]
    fn proc_sort_from_cli_invalid_returns_error() {
        let err = ProcSort::from_cli(Some("bogus")).expect_err("unknown sort key MUST fail");
        assert!(err.to_string().contains("bogus"), "error MUST mention the bad key; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_agent_tree_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_agent_tree_csv_header_and_body() {
        let root = AgentTreeNode {
            pid: 50,
            ppid: 1,
            comm: "claude".into(),
            family: Some("claude"),
            children: vec![AgentTreeNode {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                family: None,
                children: vec![],
            }],
        };
        let rss_by_pid = HashMap::from([(50, 1_000_000), (51, 500_000)]);
        let fd_by_pid = HashMap::from([(50, 10), (51, 5)]);
        let state_by_pid = HashMap::from([(50, 'R'), (51, 'S')]);
        let csv = render_agent_tree_csv(&[root], &rss_by_pid, &fd_by_pid, &state_by_pid);
        assert!(csv.starts_with(
            "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"
        ));
        assert!(csv.contains("0,0,50,1,claude,claude,R,1000000,"));
        assert!(csv.contains("0,1,51,50,,node,S,500000,"));
    }

    #[test]
    fn render_agent_tree_csv_empty_forests() {
        let csv = render_agent_tree_csv(&[], &HashMap::new(), &HashMap::new(), &HashMap::new());
        assert!(csv.starts_with(
            "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"
        ));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "empty forests MUST have only header; got {} lines",
            lines.len()
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcSort FromStr edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_sort_from_str_case_insensitive() {
        assert_eq!("PID".parse::<ProcSort>().unwrap(), ProcSort::Pid);
        assert_eq!("RSS".parse::<ProcSort>().unwrap(), ProcSort::Rss);
        assert_eq!("Fd".parse::<ProcSort>().unwrap(), ProcSort::Fd);
    }

    #[test]
    fn proc_sort_from_str_fd_and_state_parse() {
        assert_eq!("fd".parse::<ProcSort>().unwrap(), ProcSort::Fd);
        assert_eq!("state".parse::<ProcSort>().unwrap(), ProcSort::State);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: agent_row_from_watch with missing state
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn agent_row_from_watch_missing_state_is_empty() {
        let row = agent_row_from_watch(
            &DetectedAgentWatch {
                agent: DetectedAgent { pid: 99, family: "claude", comm: "claude".into() },
                resource: AgentResourceSample { mem_rss_bytes: 1024, fd_count: None },
            },
            &HashMap::new(), // no state entries
        );
        assert_eq!(row.state, "", "missing state MUST produce empty string");
        assert_eq!(row.fd_count, None);
        assert_eq!(row.mem_rss_bytes, 1024);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_proc_limit edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_proc_limit_none_returns_none() {
        assert!(parse_proc_limit(None).unwrap().is_none());
    }

    #[test]
    fn parse_proc_limit_valid_number() {
        assert_eq!(parse_proc_limit(Some(1)).unwrap(), Some(1));
        assert_eq!(parse_proc_limit(Some(100)).unwrap(), Some(100));
    }
}
