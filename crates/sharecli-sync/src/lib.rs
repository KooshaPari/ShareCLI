//! Synchronous PID → name index modeled after sharecli `ProcessPool`'s
//! `processes: RwLock<HashMap<u32, …>>` map (C00 L7).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(loom))]
use std::sync::RwLock;

#[cfg(loom)]
use loom::sync::RwLock;

/// Lock-backed registry of managed process ids.
#[derive(Debug, Default)]
pub struct PoolIndex {
    names: RwLock<HashMap<u32, String>>,
}

impl PoolIndex {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `pid` when absent. Returns `true` when inserted.
    pub fn insert(&self, pid: u32, name: impl Into<String>) -> bool {
        self.names
            .write()
            .expect("pool index write lock poisoned")
            .insert(pid, name.into())
            .is_none()
    }

    /// Remove `pid` when present. Returns `true` when removed.
    pub fn remove(&self, pid: u32) -> bool {
        self.names.write().expect("pool index write lock poisoned").remove(&pid).is_some()
    }

    /// Number of tracked pids.
    pub fn count(&self) -> usize {
        self.names.read().expect("pool index read lock poisoned").len()
    }

    /// Snapshot of tracked pids (sorted for deterministic assertions).
    pub fn pids_sorted(&self) -> Vec<u32> {
        let mut pids: Vec<u32> =
            self.names.read().expect("pool index read lock poisoned").keys().copied().collect();
        pids.sort_unstable();
        pids
    }
}

/// Relaxed counter used by sharecli metrics hot paths (loom-smoke target).
#[derive(Debug, Default)]
pub struct RelaxedCounter {
    v: AtomicU64,
}

impl RelaxedCounter {
    /// Construct a counter starting at zero.
    pub fn new() -> Self {
        Self { v: AtomicU64::new(0) }
    }

    /// Increment the counter by one (relaxed ordering).
    pub fn inc(&self) {
        self.v.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the current counter value (relaxed load).
    pub fn get(&self) -> u64 {
        self.v.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_remove_roundtrip() {
        let idx = PoolIndex::new();
        assert!(idx.insert(42, "sleep"));
        assert_eq!(idx.count(), 1);
        assert!(!idx.insert(42, "dup"));
        assert!(idx.remove(42));
        assert_eq!(idx.count(), 0);
    }
}
