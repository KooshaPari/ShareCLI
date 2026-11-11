//! Sort keys for `sharecli proc` inventory rows (AC-006.19, AC-006.36, AC-006.41).
//!
//! [`ProcSort`] is the per-inventory sort key parsed from `--sort` and applied
//! by [`crate::commands::proc::scan::sort_watched_agents`] /
//! [`crate::commands::proc::scan::sort_agent_forests`].
//! `parse_proc_limit` validates `--limit`; `limit_watched_agents` and
//! `limit_agent_forests` cap the post-sort row/forest lists.

use std::collections::HashMap;

use anyhow::{bail, Result};
use sharecli_fleet::{AgentTreeNode, DetectedAgentWatch};

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
pub(crate) fn state_sort_letter(state_by_pid: &HashMap<u32, char>, pid: u32) -> char {
    state_by_pid.get(&pid).copied().unwrap_or(char::MAX)
}
