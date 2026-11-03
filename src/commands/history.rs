// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli history` — recent CLI invocation log (FR-003 / C09 L81.12).
//!
//! Records every CLI invocation as a JSONL line in the state directory.
//! The `history` subcommand reads and displays the last N entries.
//! Supports `--json` for machine-readable output and `--clear` to truncate.

// All public functions in this module are dispatched via the CLI subcommand
// enum, so the compiler sees them as unused. Suppress the warning.
#![allow(dead_code)]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A single CLI invocation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unix timestamp (seconds since epoch).
    pub ts: u64,
    /// Top-level command name (e.g. "ps", "serve", "status").
    pub command: String,
    /// Full argument string (space-joined).
    pub args: String,
    /// Process exit code (0 = success).
    pub exit_code: i32,
}

impl HistoryEntry {
    /// Create a new entry with the current timestamp.
    pub fn now(command: &str, args: &str, exit_code: i32) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        Self { ts, command: command.to_string(), args: args.to_string(), exit_code }
    }
}

/// Return the path to the JSONL history file.
///
/// Uses `$XDG_STATE_HOME/sharecli/history.jsonl` or falls back to
/// `~/.local/state/sharecli/history.jsonl`.
pub fn history_path() -> PathBuf {
    let base = std::env::var("XDG_STATE_HOME").ok().map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".local").join("state")
    });
    base.join("sharecli").join("history.jsonl")
}

/// Append a history entry to the JSONL file (idempotent, never panics).
#[allow(dead_code)]
pub fn append(entry: &HistoryEntry) {
    append_to(entry, &history_path());
}

/// Append a history entry to a specific path.
pub fn append_to(entry: &HistoryEntry, path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = serde_json::to_writer(&mut file, entry);
        let _ = writeln!(file);
    }
}

/// Read the last `limit` entries from the history file.
/// Returns an empty vec if the file does not exist (fresh install).
pub fn read_recent(path: &Path, limit: usize) -> Result<Vec<HistoryEntry>> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).context("open history file"),
    };
    let reader = BufReader::new(file);
    let mut entries: Vec<HistoryEntry> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect();
    // Return the most recent `limit` entries (file is append-only, so tail).
    let len = entries.len();
    if len > limit {
        entries = entries[len - limit..].to_vec();
    }
    Ok(entries)
}

/// Clear the history file (creates parent dirs and empty file if absent).
pub fn clear(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, "").context("clear history file")?;
    Ok(())
}

/// Format a single entry for human-readable display.
pub fn format_entry(entry: &HistoryEntry) -> String {
    let dt = chrono_from_epoch(entry.ts);
    let status = if entry.exit_code == 0 { "ok" } else { "ERR" };
    format!(
        "{dt}  {status:>3}  sharecli {cmd} {args}",
        dt = dt,
        status = status,
        cmd = entry.command,
        args = entry.args,
    )
}

/// Very simple epoch → "YYYY-MM-DD HH:MM:SS" without pulling in chrono.
fn chrono_from_epoch(secs: u64) -> String {
    // Civil date from epoch seconds (UTC).
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    // Days since 1970-01-01 → Y/M/D (simplified calendar math).
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days: [u64; 12] =
        [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m_idx = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m_idx = i as u64;
            break;
        }
        remaining -= md;
    }
    format!(
        "{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}",
        y = y,
        mo = m_idx + 1,
        d = remaining + 1,
        h = h,
        m = m,
        s = s,
    )
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_entry_now_has_valid_timestamp() {
        let entry = HistoryEntry::now("ps", "--json", 0);
        assert!(entry.ts > 1_700_000_000); // after 2023
        assert_eq!(entry.command, "ps");
        assert_eq!(entry.args, "--json");
        assert_eq!(entry.exit_code, 0);
    }

    #[test]
    fn format_entry_contains_command() {
        let entry = HistoryEntry {
            ts: 1_700_000_000,
            command: "status".to_string(),
            args: String::new(),
            exit_code: 0,
        };
        let formatted = format_entry(&entry);
        assert!(formatted.contains("sharecli status"));
        assert!(formatted.contains("ok"));
    }

    #[test]
    fn format_entry_shows_error_for_nonzero() {
        let entry = HistoryEntry {
            ts: 1_700_000_000,
            command: "serve".to_string(),
            args: "--port 9000".to_string(),
            exit_code: 1,
        };
        let formatted = format_entry(&entry);
        assert!(formatted.contains("ERR"));
    }

    #[test]
    fn chrono_from_epoch_basic() {
        assert_eq!(chrono_from_epoch(0), "1970-01-01 00:00:00");
        assert_eq!(chrono_from_epoch(86400), "1970-01-02 00:00:00");
        assert_eq!(chrono_from_epoch(1_700_000_000), "2023-11-14 22:13:20");
    }

    #[test]
    fn read_recent_empty_file() {
        let dir = std::env::temp_dir().join("sharecli_history_test_empty");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("history.jsonl");
        fs::write(&path, "").unwrap();
        let entries = read_recent(&path, 10).unwrap();
        assert!(entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_recent_limit() {
        let dir = std::env::temp_dir().join("sharecli_history_test_limit");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("history.jsonl");
        let mut content = String::new();
        for i in 0..20 {
            let entry = HistoryEntry::now(&format!("cmd{}", i), "", 0);
            content.push_str(&serde_json::to_string(&entry).unwrap());
            content.push('\n');
        }
        fs::write(&path, &content).unwrap();
        let entries = read_recent(&path, 5).unwrap();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].command, "cmd15");
        assert_eq!(entries[4].command, "cmd19");
        let _ = fs::remove_dir_all(&dir);
    }
}
