// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Operator-facing Maildir depth probes for `sharecli status` / thermal TUI (FR-010 / AC-010.11).

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::MaildirStatus;

/// Env override for the default mesh Maildir queue root (`SHARECLI_MESH_QUEUE`).
pub const MESH_QUEUE_ENV: &str = "SHARECLI_MESH_QUEUE";

/// Resolve the mesh Maildir queue root: `SHARECLI_MESH_QUEUE` or `{state_dir}/mesh/queue`.
pub fn resolve_mesh_queue_path() -> PathBuf {
    if let Ok(raw) = std::env::var(MESH_QUEUE_ENV) {
        if !raw.is_empty() {
            return PathBuf::from(raw);
        }
    }
    default_mesh_queue_path()
}

fn default_mesh_queue_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("sharecli").join("mesh").join("queue");
        }
    }
    dirs::home_dir()
        .map(|home| home.join(".local").join("state").join("sharecli").join("mesh").join("queue"))
        .unwrap_or_else(|| PathBuf::from(".local/state/sharecli/mesh/queue"))
}

/// Read Maildir depth without creating queue directories.
pub fn capture_maildir_status() -> Result<Option<MaildirStatus>> {
    MaildirStatus::probe(resolve_mesh_queue_path())
}

impl MaildirStatus {
    /// Probe an on-disk Maildir queue; returns `None` when the path is absent or not a queue.
    pub fn probe(path: impl Into<PathBuf>) -> Result<Option<Self>> {
        let path = path.into();
        if !path.is_dir() {
            return Ok(None);
        }
        let new = path.join("new");
        let cur = path.join("cur");
        if !new.exists() && !cur.exists() {
            return Ok(None);
        }
        let ready = count_queue_files(&new)?;
        let in_flight = count_queue_files(&cur)?;
        Ok(Some(Self { path, ready, in_flight, pending: ready + in_flight }))
    }

    /// Operator-facing status block for `sharecli status` (FR-010 / AC-010.11).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== Mesh Maildir Queue ===\n\n");
        out.push_str(&format!("Path:      {}\n", self.path.display()));
        out.push_str(&format!(
            "Ready:     {}\nIn-flight: {}\nPending:   {}\n",
            self.ready, self.in_flight, self.pending
        ));
        out
    }
}

fn count_queue_files(directory: &Path) -> Result<usize> {
    if !directory.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::MaildirQueue;

    #[test]
    fn probe_absent_path_is_none() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("no-queue");
        assert!(MaildirStatus::probe(&missing).unwrap().is_none());
    }

    #[test]
    fn probe_counts_ready_and_in_flight() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        let id = q.enqueue(json!({"op": "mesh"}), 3).unwrap();
        q.claim(Some("agent-a")).unwrap();
        q.enqueue(json!({"op": "wait"}), 5).unwrap();

        let st = MaildirStatus::probe(dir.path()).unwrap().expect("probe");
        assert_eq!(st.ready, 1);
        assert_eq!(st.in_flight, 1);
        assert_eq!(st.pending, 2);
        assert!(!dir.path().join("new").join(&id).exists());
        let section = st.format_status_section();
        assert!(section.contains("=== Mesh Maildir Queue ==="));
        assert!(section.contains("Ready:"));
        assert!(section.contains("In-flight:"));
        assert!(section.contains("Pending:"));
    }

    #[test]
    fn capture_honors_mesh_queue_env() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        q.enqueue(json!({"op": "env"}), 0).unwrap();
        unsafe {
            std::env::set_var(MESH_QUEUE_ENV, dir.path());
        }
        let st = capture_maildir_status().unwrap().expect("captured");
        assert_eq!(st.ready, 1);
        unsafe {
            std::env::remove_var(MESH_QUEUE_ENV);
        }
    }
}
