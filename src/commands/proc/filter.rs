//! Inventory filter for `sharecli proc` (AC-006.17, AC-006.25, AC-006.27–31, AC-006.38).
//!
//! [`ProcFilter`] is the per-inventory filter carried by every `proc` row.
//! `from_cli` validates the CLI aggregation (mutual-exclusion, min≤max bounds)
//! and the helper `comm_matches_pattern` / `cmdline_matches_pattern` are
//! case-insensitive substring matches used by [`crate::commands::proc::scan`].

use anyhow::{bail, Context, Result};

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
            Some(raw) => Some(sharecli_fleet::parse_rss_bytes(&raw, "--min-rss")?),
        };
        let max_rss_bytes = match max_rss {
            None => None,
            Some(raw) => Some(sharecli_fleet::parse_rss_bytes(&raw, "--max-rss")?),
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

    pub(crate) fn active(&self) -> bool {
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
