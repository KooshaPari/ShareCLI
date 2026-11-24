// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Device-scoped registry of sharecli-managed processes.
//!
//! Each `sharecli` invocation constructs its own [`ProcessPool`], whose handle
//! map is in-memory only. That makes the pool an authority for exactly one
//! process: `sharecli start` records a handle that the next `sharecli stop`
//! invocation cannot see, so the stop is a silent no-op on the device.
//!
//! This registry is the device-level authority that closes that gap. `spawn`
//! appends an entry, `kill` removes one, and `list` / `find` / `kill` on a fresh
//! pool read back what earlier invocations started.
//!
//! Scope and limits, stated plainly:
//!
//! - Entries are written atomically (temp file + rename) but the registry is not
//!   a lock: two invocations spawning at the same instant can drop one entry
//!   (last writer wins). A resident supervisor removes that race entirely; this
//!   file is the bounded step that makes cross-invocation kill work today.
//! - Liveness is probed with `kill(pid, 0)`, so a recycled PID is possible in
//!   principle. The recorded `started_at` and `cmd` let a caller discriminate.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One process recorded by a `sharecli` invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub pid: u32,
    pub project: Option<String>,
    pub harness: Option<String>,
    pub cmd: Vec<String>,
    /// Unix epoch seconds, used to tell a live process from a recycled PID.
    pub started_at: u64,
}

impl RegistryEntry {
    pub fn new(
        pid: u32,
        project: Option<String>,
        harness: Option<String>,
        cmd: Vec<String>,
    ) -> Self {
        Self { pid, project, harness, cmd, started_at: now_secs() }
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// True when `pid` exists. `kill(pid, 0)` performs the permission and existence
/// check without delivering a signal.
pub fn pid_alive(pid: u32) -> bool {
    // PID 0 is not a real process handle: `kill(0, 0)` targets the caller's
    // whole process group, so it always "succeeds". Treat it as dead.
    if pid == 0 {
        return false;
    }
    // SAFETY: signal 0 delivers nothing; it only validates the pid.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Signal a pid, preferring its process group so a harness's own children go
/// with it (sharecli spawns each harness as a group leader).
pub fn signal_group_or_pid(pid: u32, signal: i32) {
    // SAFETY: the target is a pid sharecli recorded at spawn time.
    let as_group = unsafe { libc::kill(-(pid as libc::pid_t), signal) };
    if as_group != 0 {
        // SAFETY: same pid, single-process fallback for a non-group child.
        unsafe { libc::kill(pid as libc::pid_t, signal) };
    }
}

/// Handle to the on-disk registry.
#[derive(Debug, Clone)]
pub struct Registry {
    path: PathBuf,
}

impl Default for Registry {
    fn default() -> Self {
        Self::open()
    }
}

impl Registry {
    /// Registry at the standard per-device location.
    pub fn open() -> Self {
        Self { path: default_path() }
    }

    /// Registry at an explicit path (used by tests and by `SHARECLI_STATE_DIR`).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read every recorded entry, ignoring a corrupt or absent file.
    pub fn entries(&self) -> Vec<RegistryEntry> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Entries whose process is still alive. Dead entries are dropped from the
    /// file as a side effect so the registry cannot grow without bound.
    pub fn prune_dead(&self) -> Vec<RegistryEntry> {
        let all = self.entries();
        let (live, dead): (Vec<_>, Vec<_>) = all.into_iter().partition(|e| pid_alive(e.pid));
        if !dead.is_empty() {
            let _ = self.write(&live);
        }
        live
    }

    /// Record a spawned process. Idempotent per pid.
    pub fn insert(&self, entry: RegistryEntry) -> Result<()> {
        let mut entries = self.entries();
        entries.retain(|e| e.pid != entry.pid && pid_alive(e.pid));
        entries.push(entry);
        self.write(&entries)
    }

    /// Remove a pid. Returns whether anything was recorded for it.
    pub fn remove(&self, pid: u32) -> bool {
        let mut entries = self.entries();
        let before = entries.len();
        entries.retain(|e| e.pid != pid);
        let removed = entries.len() != before;
        if removed {
            let _ = self.write(&entries);
        }
        removed
    }

    pub fn contains(&self, pid: u32) -> bool {
        self.entries().iter().any(|e| e.pid == pid)
    }

    fn write(&self, entries: &[RegistryEntry]) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("create registry dir {}", dir.display()))?;
        }
        let body = serde_json::to_string_pretty(entries)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, body)
            .with_context(|| format!("write registry tmp {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("publish registry {}", self.path.display()))?;
        Ok(())
    }
}

/// Device state root shared by the process registry and the build-slot pool.
/// `SHARECLI_STATE_DIR` overrides it so tests and the acceptance harness never
/// touch the operator's real state.
pub(crate) fn state_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("SHARECLI_STATE_DIR") {
        return PathBuf::from(dir);
    }
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("sharecli")
}

/// `<state dir>/sharecli/processes.json`. `SHARECLI_STATE_DIR` overrides it so
/// tests and the acceptance harness never touch the operator's real registry.
fn default_path() -> PathBuf {
    state_root().join("processes.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry(name: &str) -> Registry {
        let dir = std::env::temp_dir().join(format!("sharecli-registry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Registry::at(dir.join("processes.json"))
    }

    #[test]
    fn insert_then_read_back() {
        let registry = temp_registry("roundtrip");
        let entry = RegistryEntry::new(
            std::process::id(),
            Some("proj".into()),
            Some("cargo".into()),
            vec!["cargo".into(), "build".into()],
        );
        registry.insert(entry.clone()).expect("insert");
        assert_eq!(registry.entries(), vec![entry]);
    }

    #[test]
    fn remove_clears_entry() {
        let registry = temp_registry("remove");
        let pid = std::process::id();
        registry
            .insert(RegistryEntry::new(pid, None, None, vec!["sleep".into()]))
            .expect("insert");
        assert!(registry.contains(pid));
        assert!(registry.remove(pid));
        assert!(!registry.contains(pid));
    }

    #[test]
    fn prune_drops_dead_pids_and_keeps_live_ones() {
        let registry = temp_registry("prune");
        let live = std::process::id();
        registry
            .insert(RegistryEntry::new(live, None, None, vec!["live".into()]))
            .expect("insert live");
        // PID 0 is never a user process, so it is a stable "dead" id.
        registry
            .insert(RegistryEntry::new(0, None, None, vec!["dead".into()]))
            .expect("insert dead");
        let pruned = registry.prune_dead();
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].pid, live);
        assert_eq!(registry.entries().len(), 1);
    }

    #[test]
    fn corrupt_file_reads_as_empty_not_panic() {
        let registry = temp_registry("corrupt");
        if let Some(dir) = registry.path().parent() {
            std::fs::create_dir_all(dir).expect("mkdir");
        }
        std::fs::write(registry.path(), "{not json").expect("write");
        assert!(registry.entries().is_empty());
    }
}
