// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Maildir-style teammate task queue (ported from `thegent.mesh.task_queue`).
//!
//! All file operations use atomic rename. The queue is crash-recoverable: no
//! in-memory state is required. Tasks stranded in `cur/` after a crash remain
//! visible via [`MaildirQueue::list_pending`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Operator-facing Maildir depth snapshot (`sharecli mesh status`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaildirStatus {
    /// Queue root path.
    pub path: PathBuf,
    /// Tasks waiting in `new/` (ready to claim).
    pub ready: usize,
    /// Tasks claimed in `cur/` (in-flight).
    pub in_flight: usize,
    /// `ready + in_flight` (same as [`MaildirQueue::list_pending`] length).
    pub pending: usize,
}

/// Task envelope stored as JSON under `tmp/` / `new/` / `cur/`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskEnvelope {
    pub id: String,
    pub payload: Value,
    /// Integer 0–9; lower = higher priority (matches thegent).
    pub priority: u8,
    pub created_at: f64,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Filesystem-backed Maildir task queue.
///
/// Directory layout::
///
/// ```text
/// <path>/
///   tmp/   # staging; write then rename → new/
///   new/   # ready to claim
///   cur/   # claimed / in-flight
/// ```
pub struct MaildirQueue {
    path: PathBuf,
    tmp: PathBuf,
    new: PathBuf,
    cur: PathBuf,
}

impl MaildirQueue {
    /// Create (or open) a queue rooted at `path`, ensuring `tmp`/`new`/`cur`.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let tmp = path.join("tmp");
        let new = path.join("new");
        let cur = path.join("cur");
        for d in [&tmp, &new, &cur] {
            fs::create_dir_all(d).with_context(|| format!("create maildir dir {}", d.display()))?;
        }
        Ok(Self { path, tmp, new, cur })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enqueue `payload` at `priority` (0–9; lower first). Returns task id.
    ///
    /// Lifecycle: write `tmp/<id>` → rename → `new/<id>`.
    pub fn enqueue(&self, payload: Value, priority: u8) -> Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();
        let envelope = TaskEnvelope {
            id: task_id.clone(),
            payload,
            priority: priority.min(9),
            created_at: now,
            attempts: 0,
            owner: None,
        };
        let bytes = serde_json::to_vec_pretty(&envelope).context("serialize task envelope")?;
        let tmp_path = self.tmp.join(&task_id);
        let new_path = self.new.join(&task_id);
        fs::write(&tmp_path, &bytes)
            .with_context(|| format!("write tmp task {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &new_path)
            .with_context(|| format!("rename tmp→new for {}", task_id))?;
        Ok(task_id)
    }

    /// Claim (dequeue) the highest-priority ready task.
    ///
    /// Lifecycle: rename `new/<id>` → `cur/<id>`, bump `attempts`, optional owner.
    /// Returns `None` when `new/` is empty.
    pub fn claim(&self, owner: Option<&str>) -> Result<Option<TaskEnvelope>> {
        let mut candidates = self.list_envelopes(&self.new)?;
        if candidates.is_empty() {
            return Ok(None);
        }
        candidates.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then(a.created_at.partial_cmp(&b.created_at).unwrap_or(std::cmp::Ordering::Equal))
        });

        for mut envelope in candidates {
            let new_path = self.new.join(&envelope.id);
            let cur_path = self.cur.join(&envelope.id);
            match fs::rename(&new_path, &cur_path) {
                Ok(()) => {
                    envelope.attempts = envelope.attempts.saturating_add(1);
                    if let Some(o) = owner {
                        envelope.owner = Some(o.to_string());
                    }
                    let bytes =
                        serde_json::to_vec_pretty(&envelope).context("serialize claimed task")?;
                    fs::write(&cur_path, bytes)
                        .with_context(|| format!("update cur task {}", cur_path.display()))?;
                    return Ok(Some(envelope));
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(e).with_context(|| format!("claim rename {}", envelope.id));
                }
            }
        }
        Ok(None)
    }

    /// Alias for [`Self::claim`] (thegent `dequeue` name).
    pub fn dequeue(&self, owner: Option<&str>) -> Result<Option<TaskEnvelope>> {
        self.claim(owner)
    }

    /// Acknowledge success: remove `cur/<id>` (idempotent).
    pub fn ack(&self, task_id: &str) -> Result<()> {
        let cur_path = self.cur.join(task_id);
        match fs::remove_file(&cur_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| format!("ack remove {}", cur_path.display())),
        }
    }

    /// Negative-ack: return `cur/<id>` → `new/<id>` for retry (idempotent).
    pub fn nack(&self, task_id: &str) -> Result<()> {
        let cur_path = self.cur.join(task_id);
        let new_path = self.new.join(task_id);
        match fs::rename(&cur_path, &new_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| format!("nack rename {}", task_id)),
        }
    }

    /// List pending tasks from both `new/` and `cur/` (unsorted).
    pub fn list_pending(&self) -> Result<Vec<TaskEnvelope>> {
        let mut out = self.list_envelopes(&self.new)?;
        out.extend(self.list_envelopes(&self.cur)?);
        Ok(out)
    }

    /// Reclaim in-flight tasks owned by `owner` back to `new/`.
    pub fn reclaim_owner(&self, owner: &str) -> Result<usize> {
        let mut reclaimed = 0usize;
        for entry in
            fs::read_dir(&self.cur).with_context(|| format!("read cur {}", self.cur.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let data = match fs::read_to_string(entry.path()) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let envelope: TaskEnvelope = match serde_json::from_str(&data) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if envelope.owner.as_deref() != Some(owner) {
                continue;
            }
            let new_path = self.new.join(entry.file_name());
            match fs::rename(entry.path(), &new_path) {
                Ok(()) => reclaimed += 1,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e).context("reclaim rename"),
            }
        }
        Ok(reclaimed)
    }

    /// Snapshot queue depth for operator status (`new/` ready, `cur/` in-flight).
    pub fn status(&self) -> Result<MaildirStatus> {
        let ready = self.list_envelopes(&self.new)?;
        let in_flight = self.list_envelopes(&self.cur)?;
        Ok(MaildirStatus {
            path: self.path.clone(),
            ready: ready.len(),
            in_flight: in_flight.len(),
            pending: ready.len() + in_flight.len(),
        })
    }

    fn list_envelopes(&self, directory: &Path) -> Result<Vec<TaskEnvelope>> {
        let mut results = Vec::new();
        let entries = match fs::read_dir(directory) {
            Ok(rd) => rd,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(results),
            Err(e) => return Err(e).with_context(|| format!("read {}", directory.display())),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let data = match fs::read_to_string(entry.path()) {
                Ok(s) => s,
                Err(_) => continue,
            };
            match serde_json::from_str::<TaskEnvelope>(&data) {
                Ok(env) => results.push(env),
                Err(_) => continue,
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn enqueue_claim_ack_lifecycle() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        let id = q.enqueue(json!({"cmd": "lint"}), 5).unwrap();
        assert!(dir.path().join("new").join(&id).exists());

        let claimed = q.claim(Some("agent-a")).unwrap().expect("claimed");
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.attempts, 1);
        assert_eq!(claimed.owner.as_deref(), Some("agent-a"));
        assert!(dir.path().join("cur").join(&id).exists());
        assert!(!dir.path().join("new").join(&id).exists());

        q.ack(&id).unwrap();
        assert!(!dir.path().join("cur").join(&id).exists());
        assert!(q.list_pending().unwrap().is_empty());
    }

    #[test]
    fn priority_ordering() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        q.enqueue(json!(1), 9).unwrap();
        q.enqueue(json!(0), 0).unwrap();
        let first = q.claim(None).unwrap().unwrap();
        assert_eq!(first.priority, 0);
        assert_eq!(first.payload, json!(0));
    }

    #[test]
    fn nack_returns_to_new() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        let id = q.enqueue(json!({}), 3).unwrap();
        q.claim(None).unwrap().unwrap();
        q.nack(&id).unwrap();
        assert!(dir.path().join("new").join(&id).exists());
        assert!(!dir.path().join("cur").join(&id).exists());
    }

    #[test]
    fn status_counts_ready_and_in_flight() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        q.enqueue(json!(1), 1).unwrap();
        q.enqueue(json!(2), 2).unwrap();
        q.claim(Some("w1")).unwrap().unwrap();
        let st = q.status().unwrap();
        assert_eq!(st.ready, 1);
        assert_eq!(st.in_flight, 1);
        assert_eq!(st.pending, 2);
        assert_eq!(st.path, dir.path());
    }

    #[test]
    fn reclaim_owner_returns_cur_to_new() {
        let dir = TempDir::new().unwrap();
        let q = MaildirQueue::open(dir.path()).unwrap();
        let id = q.enqueue(json!({"op": "x"}), 1).unwrap();
        q.claim(Some("agent-dead")).unwrap().unwrap();
        assert_eq!(q.reclaim_owner("other").unwrap(), 0);
        assert_eq!(q.reclaim_owner("agent-dead").unwrap(), 1);
        assert!(dir.path().join("new").join(&id).exists());
        assert!(!dir.path().join("cur").join(&id).exists());
        let st = q.status().unwrap();
        assert_eq!(st.ready, 1);
        assert_eq!(st.in_flight, 0);
    }
}
