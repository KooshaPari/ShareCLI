// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Append-only JSONL audit log for security-relevant serve events.
//!
//! Default path: `$XDG_STATE_HOME/sharecli/audit.jsonl` or
//! `~/.local/state/sharecli/audit.jsonl` (Windows: `%LOCALAPPDATA%/sharecli/audit.jsonl`).
//! Override with `SHARECLI_AUDIT_LOG`.
//!
//! Size-based retention: when the active file exceeds
//! `SHARECLI_AUDIT_MAX_BYTES` (default 10 MiB), it rotates to
//! `audit.jsonl.1` … `audit.jsonl.N` where N = `SHARECLI_AUDIT_RETAIN`
//! (default 5). Oldest files beyond N are deleted.
//!
//! Failures to write are logged via `tracing::warn` and never fail the request
//! path (best-effort audit).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tracing::warn;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

const DEFAULT_MAX_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_RETAIN: usize = 5;

/// True when spawn/serve audit rows are enabled via explicit `SHARECLI_AUDIT_LOG`.
pub fn is_configured() -> bool {
    std::env::var("SHARECLI_AUDIT_LOG").map(|s| !s.is_empty()).unwrap_or(false)
}

/// Emit one audit event (`event` + arbitrary JSON fields).
pub fn emit(event: &str, fields: Value) {
    emit_to(&audit_log_path(), event, fields);
}

/// Emit only when [`is_configured`] — soft gate for spawn/stop lifecycle rows.
pub fn emit_if_configured(event: &str, fields: Value) {
    if is_configured() {
        emit(event, fields);
    }
}

/// Emit to an explicit path (tests + callers that already resolved the path).
pub fn emit_to(path: &Path, event: &str, fields: Value) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!(error = %e, path = %path.display(), "audit log: create_dir_all failed");
            return;
        }
    }

    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);

    let mut record = json!({
        "ts": ts,
        "event": event,
        "service": "sharecli",
    });
    if let (Some(obj), Some(fields_obj)) = (record.as_object_mut(), fields.as_object()) {
        for (k, v) in fields_obj {
            obj.insert(k.clone(), v.clone());
        }
    }

    let line = match serde_json::to_string(&record) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "audit log: serialize failed");
            return;
        }
    };

    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    rotate_if_needed(path);
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => {
            if let Err(e) = writeln!(f, "{line}") {
                warn!(error = %e, path = %path.display(), "audit log: write failed");
            }
        }
        Err(e) => {
            warn!(error = %e, path = %path.display(), "audit log: open failed");
        }
    }
}

/// Resolve the audit log file path.
pub fn audit_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("SHARECLI_AUDIT_LOG") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    state_dir().join("audit.jsonl")
}

fn max_bytes() -> u64 {
    std::env::var("SHARECLI_AUDIT_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_BYTES)
}

fn retain_count() -> usize {
    std::env::var("SHARECLI_AUDIT_RETAIN")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_RETAIN)
}

/// Rotate `path` when it exceeds the configured size budget.
///
/// Callers must hold [`WRITE_LOCK`].
fn rotate_if_needed(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() < max_bytes() {
        return;
    }
    let retain = retain_count();
    let oldest = rotation_path(path, retain);
    let _ = std::fs::remove_file(&oldest);
    for i in (1..retain).rev() {
        let from = rotation_path(path, i);
        let to = rotation_path(path, i + 1);
        let _ = std::fs::rename(&from, &to);
    }
    if let Err(e) = std::fs::rename(path, rotation_path(path, 1)) {
        warn!(error = %e, path = %path.display(), "audit log: rotate rename failed");
    }
}

fn rotation_path(path: &Path, index: usize) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{index}"));
    PathBuf::from(s)
}

fn state_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("sharecli");
        }
    }
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("sharecli");
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("state")
        .join("sharecli")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Serializes all audit tests that mutate process-global env vars.
    /// The tests otherwise race on `SHARECLI_AUDIT_LOG` (e.g. one test
    /// removes the var while another is mid-emit, so the file is never
    /// written and `read_to_string` fails with `NotFound`).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn emit_if_configured_respects_env_gate() {
        let _env = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gated.jsonl");
        unsafe {
            std::env::remove_var("SHARECLI_AUDIT_LOG");
        }
        emit_if_configured("spawn", json!({ "outcome": "ok" }));
        assert!(!path.exists());

        unsafe {
            std::env::set_var("SHARECLI_AUDIT_LOG", &path);
        }
        emit_if_configured("spawn", json!({ "outcome": "ok" }));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"event\":\"spawn\""));
        unsafe {
            std::env::remove_var("SHARECLI_AUDIT_LOG");
        }
    }

    #[test]
    fn emit_appends_json_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        emit_to(&path, "test_event", json!({ "k": 1 }));
        emit_to(&path, "test_event", json!({ "k": 2 }));
        let body = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"event\":\"test_event\""));
        assert!(lines[0].contains("\"k\":1"));
    }

    #[test]
    fn path_respects_env_override() {
        let _env = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.jsonl");
        unsafe {
            std::env::set_var("SHARECLI_AUDIT_LOG", &path);
        }
        assert_eq!(audit_log_path(), path);
        unsafe {
            std::env::remove_var("SHARECLI_AUDIT_LOG");
        }
    }

    #[test]
    fn rotates_when_over_max_bytes() {
        let _env = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        unsafe {
            std::env::set_var("SHARECLI_AUDIT_MAX_BYTES", "80");
            std::env::set_var("SHARECLI_AUDIT_RETAIN", "2");
        }
        for i in 0..6 {
            emit_to(&path, "rot", json!({ "i": i, "pad": "xxxxxxxxxxxxxxxxxxxx" }));
        }
        assert!(path.exists() || rotation_path(&path, 1).exists());
        assert!(rotation_path(&path, 1).exists() || rotation_path(&path, 2).exists());
        assert!(!rotation_path(&path, 3).exists());
        unsafe {
            std::env::remove_var("SHARECLI_AUDIT_MAX_BYTES");
            std::env::remove_var("SHARECLI_AUDIT_RETAIN");
        }
    }
}
