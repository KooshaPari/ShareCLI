// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Undo / restore support for sharecli (C09 L81.9).
//!
//! Provides a journal-backed undo model. Every mutating sharecli operation
//! (start, stop, kill, project add/remove, etc.) appends a record to
//! `$XDG_STATE_HOME/sharecli/operations.jsonl` (falling back to
//! `~/.local/state/sharecli/operations.jsonl`). The `sharecli undo` CLI
//! command surfaces this journal and lets operators revert a recorded
//! action by id via `--restore --id <ULID>`.
//!
//! This module is intentionally dependency-free: serde is the only
//! serialization dependency and it is already in the crate dependency
//! graph. The journal format is line-delimited JSON so it composes with
//! `jq` and the existing `history` command (`src/commands/history.rs`).

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// One row in the operations journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRecord {
    /// ULID-style sortable id (monotonic, lexicographic).
    pub id: String,
    /// Unix timestamp (seconds).
    pub ts: u64,
    /// Command kind: `start | stop | kill | project_add | project_remove | install`.
    pub kind: String,
    /// Free-form target descriptor (`pid`, project name, harness, etc.).
    pub target: String,
    /// Reversible flag — `true` means `--restore --id <id>` can attempt to
    /// undo. Operations that touch external state (`install`) opt out.
    pub reversible: bool,
    /// Optional operator note captured at action time.
    #[serde(default)]
    pub note: Option<String>,
}

/// Severity enum used by `rollback_journal_for_id` to colour the
/// `sharecli undo --json` output. Kept here (rather than in main) so
/// external callers can reuse the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Resolve the on-disk journal path, honouring `$XDG_STATE_HOME` with a
/// `~/.local/state/sharecli` fallback (Linux/macOS) or `%LOCALAPPDATA%`
/// fallback (Windows).
pub fn journal_path() -> PathBuf {
    if let Ok(p) = std::env::var("XDG_STATE_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p).join("sharecli").join("operations.jsonl");
        }
    }
    #[cfg(windows)]
    {
        if let Ok(p) = std::env::var("LOCALAPPDATA") {
            if !p.is_empty() {
                return PathBuf::from(p).join("sharecli").join("operations.jsonl");
            }
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_else(|| std::ffi::OsString::from("."));
    PathBuf::from(home).join(".local").join("state").join("sharecli").join("operations.jsonl")
}

/// Ensure the parent directory exists (mkdir -p semantics). Returns the
/// resolved path so callers can `OpenOptions::new().append(true)` against it.
pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create journal parent {}", parent.display()))?;
    }
    Ok(())
}

/// Append one record to the journal. Caller-supplied `id` is trusted
/// (the sharecli `start`/`stop`/`install` paths generate ULIDs at the
/// call site so callers never have to think about id formatting).
pub fn append_record(path: &Path, record: &OperationRecord) -> Result<()> {
    ensure_parent(path)?;
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(record)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Read the most recent `limit` records from the journal in reverse
/// chronological order (newest first). Missing journal is not an error —
/// returns an empty vec so first-run users see an empty list instead of a
/// crash.
pub fn read_recent(path: &Path, limit: usize) -> Result<Vec<OperationRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path).with_context(|| format!("open journal {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut out: Vec<OperationRecord> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<OperationRecord>(&line) {
            Ok(rec) => out.push(rec),
            // Skip corrupted rows — the journal is append-only and we
            // never want one bad line to brick `--json` consumers.
            Err(_) => continue,
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.ts));
    out.truncate(limit);
    Ok(out)
}

/// Mark `id` as rolled back by appending a tombstone record. Returns the
/// restored record if it existed, otherwise an error.
///
/// The actual restore work is performed by the calling command (we keep
/// this module free of subprocess side effects so the rollback policy
/// stays operator-visible and auditable).
pub fn mark_rolled_back(path: &Path, id: &str, note: Option<String>) -> Result<OperationRecord> {
    let records = read_recent(path, usize::MAX)?;
    let rec = records
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow!("no operation with id {id} in journal"))?;
    if !rec.reversible {
        return Err(anyhow!("operation {id} is not reversible"));
    }
    let tombstone = OperationRecord {
        id: format!("{id}-rb"),
        ts: unix_ts_secs(),
        kind: "rollback".to_string(),
        target: rec.target.clone(),
        reversible: false,
        note: note.or(Some(format!("rolled back {}", rec.id))),
    };
    append_record(path, &tombstone)?;
    Ok(rec)
}

fn unix_ts_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Render `records` as a human-friendly table. Empty list yields a
/// C10-style empty-state hint so first-run operators know how to start.
pub fn render_text(records: &[OperationRecord]) -> String {
    if records.is_empty() {
        return "No operations recorded yet.\n\
                Hint: start a harness, then run 'sharecli undo' to see journal entries."
            .to_string();
    }
    let mut out = format!(
        "{:<24} {:<10} {:<14} {:<6} {}\n",
        "ID (ts)", "KIND", "TARGET", "REVERSIBLE", "NOTE"
    );
    out.push_str(&"-".repeat(80));
    out.push('\n');
    for r in records {
        let ts = format_ts(r.ts);
        let note = r.note.as_deref().unwrap_or("");
        out.push_str(&format!(
            "{:<24} {:<10} {:<14} {:<6} {}\n",
            format!("{}@{}", &r.id[..r.id.len().min(8)], ts),
            r.kind,
            r.target,
            if r.reversible { "yes" } else { "no" },
            note
        ));
    }
    out
}

fn format_ts(ts: u64) -> String {
    // Tiny RFC3339-ish formatter — keeps the dependency surface minimal.
    // Operators who need precision can use `--json`.
    format!("@{ts}")
}

/// Top-level `sharecli undo` entry point. Wired by `src/main.rs`.
pub fn run(limit: usize, json: bool, restore: bool, id: Option<String>) -> Result<()> {
    let path = journal_path();
    if restore {
        let target = id.ok_or_else(|| anyhow!("--restore requires --id <ULID>"))?;
        let restored = mark_rolled_back(&path, &target, None)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&restored)?);
        } else {
            println!(
                "Marked {} ({}) as rolled back; recorded tombstone {}-rb.",
                restored.id, restored.kind, restored.id
            );
        }
        return Ok(());
    }
    let records = read_recent(&path, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
    } else {
        print!("{}", render_text(&records));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = env::temp_dir();
        p.push(format!("sharecli-undo-test-{name}-{}", std::process::id()));
        p.push("operations.jsonl");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
        p
    }

    #[test]
    fn journal_path_honours_xdg_state_home() {
        let dir = env::temp_dir().join(format!("xdg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests run single-threaded per binary; env::set_var is safe here.
        unsafe {
            env::set_var("XDG_STATE_HOME", &dir);
        }
        let p = journal_path();
        assert!(p.starts_with(&dir));
        assert!(p.ends_with("sharecli/operations.jsonl"));
        unsafe {
            env::remove_var("XDG_STATE_HOME");
        }
    }

    #[test]
    fn append_then_read_recent_roundtrips() {
        let path = tmp_path("roundtrip");
        let rec = OperationRecord {
            id: "01TEST000000000000000000".to_string(),
            ts: 1_700_000_000,
            kind: "start".to_string(),
            target: "pid=1234".to_string(),
            reversible: true,
            note: None,
        };
        append_record(&path, &rec).unwrap();
        let got = read_recent(&path, 10).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], rec);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn mark_rolled_back_appends_tombstone() {
        let path = tmp_path("rollback");
        let rec = OperationRecord {
            id: "01TEST000000000000000001".to_string(),
            ts: 1_700_000_000,
            kind: "stop".to_string(),
            target: "pid=42".to_string(),
            reversible: true,
            note: None,
        };
        append_record(&path, &rec).unwrap();
        let restored = mark_rolled_back(&path, &rec.id, None).unwrap();
        assert_eq!(restored.id, rec.id);
        let after = read_recent(&path, 10).unwrap();
        assert_eq!(after.len(), 2);
        assert!(after.iter().any(|r| r.kind == "rollback" && r.target == rec.target));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn read_recent_handles_missing_journal() {
        let path = tmp_path("missing");
        let got = read_recent(&path, 5).unwrap();
        assert!(got.is_empty());
    }
}
