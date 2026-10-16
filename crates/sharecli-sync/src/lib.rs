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
    pub fn new() -> Self {
        Self { v: AtomicU64::new(0) }
    }

    pub fn inc(&self) {
        self.v.fetch_add(1, Ordering::Relaxed);
    }

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

    // --- PoolIndex: empty-state baselines -------------------------------------

    #[test]
    fn pool_index_default_is_empty() {
        let idx = PoolIndex::default();
        assert_eq!(idx.count(), 0);
        assert!(idx.pids_sorted().is_empty());
    }

    #[test]
    fn pool_index_new_is_empty() {
        let idx = PoolIndex::new();
        assert_eq!(idx.count(), 0);
        assert!(idx.pids_sorted().is_empty());
    }

    // --- PoolIndex: pids_sorted ordering --------------------------------------

    #[test]
    fn pool_index_pids_sorted_returns_sorted_order() {
        let idx = PoolIndex::new();
        for pid in [7u32, 1, 5, 3, 9, 4, 2, 8, 6, 0] {
            assert!(idx.insert(pid, "proc"));
        }
        assert_eq!(idx.pids_sorted(), (0u32..=9).collect::<Vec<_>>());
        assert_eq!(idx.count(), 10);
    }

    #[test]
    fn pool_index_pids_sorted_stable_after_removal() {
        let idx = PoolIndex::new();
        for pid in 0u32..5 {
            assert!(idx.insert(pid, "proc"));
        }
        assert!(idx.remove(2));
        assert!(idx.remove(0));
        assert_eq!(idx.pids_sorted(), vec![1, 3, 4]);
        assert_eq!(idx.count(), 3);
    }

    // --- PoolIndex: insert/remove conflict paths ------------------------------

    #[test]
    fn pool_index_insert_existing_pid_returns_false() {
        let idx = PoolIndex::new();
        assert!(idx.insert(100, "first"));
        assert!(!idx.insert(100, "second"));
        assert_eq!(idx.count(), 1);
        assert!(!idx.insert(100, "third"));
        assert_eq!(idx.count(), 1);
    }

    #[test]
    fn pool_index_remove_missing_pid_returns_false() {
        let idx = PoolIndex::new();
        assert!(idx.insert(7, "alive"));
        assert!(!idx.remove(7));
        assert!(!idx.remove(99));
        assert_eq!(idx.count(), 1);
        assert!(idx.remove(7));
        assert_eq!(idx.count(), 0);
    }

    #[test]
    fn pool_index_remove_then_readd_roundtrip() {
        let idx = PoolIndex::new();
        assert!(idx.insert(11, "alpha"));
        assert!(idx.remove(11));
        assert_eq!(idx.count(), 0);
        assert!(idx.insert(11, "beta"));
        assert_eq!(idx.count(), 1);
        assert_eq!(idx.pids_sorted(), vec![11]);
    }

    // --- PoolIndex: scale and edge-case pid values ----------------------------

    #[test]
    fn pool_index_large_set_round_trip() {
        let idx = PoolIndex::new();
        const N: u32 = 256;
        for pid in 0u32..N {
            assert!(idx.insert(pid, "proc"));
        }
        assert_eq!(idx.count(), N as usize);
        let pids = idx.pids_sorted();
        assert_eq!(pids.len(), N as usize);
        assert_eq!(pids.first().copied(), Some(0));
        assert_eq!(pids.last().copied(), Some(N - 1));
        for window in pids.windows(2) {
            assert!(window[0] < window[1], "pids must be strictly ascending");
        }
    }

    #[test]
    fn pool_index_pid_range_boundaries() {
        let idx = PoolIndex::new();
        assert!(idx.insert(0, "zero"));
        assert!(idx.insert(u32::MAX, "max"));
        assert_eq!(idx.count(), 2);
        assert_eq!(idx.pids_sorted(), vec![0, u32::MAX]);
        assert!(!idx.insert(0, "zero-again"));
        assert!(!idx.insert(u32::MAX, "max-again"));
        assert_eq!(idx.count(), 2);
    }

    #[test]
    fn pool_index_empty_name_is_allowed() {
        let idx = PoolIndex::new();
        assert!(idx.insert(1, ""));
        assert_eq!(idx.count(), 1);
        assert_eq!(idx.pids_sorted(), vec![1]);
    }

    // --- RelaxedCounter: default, monotone, isolation -------------------------

    #[test]
    fn relaxed_counter_default_is_zero() {
        let counter = RelaxedCounter::default();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn relaxed_counter_new_is_zero() {
        let counter = RelaxedCounter::new();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn relaxed_counter_inc_increments() {
        let counter = RelaxedCounter::new();
        const N: u32 = 100;
        for _ in 0..N {
            counter.inc();
        }
        assert_eq!(counter.get(), N as u64);
    }

    #[test]
    fn relaxed_counter_get_is_idempotent() {
        let counter = RelaxedCounter::new();
        counter.inc();
        counter.inc();
        counter.inc();
        assert_eq!(counter.get(), 3);
        assert_eq!(counter.get(), 3);
        assert_eq!(counter.get(), 3);
    }

    #[test]
    fn relaxed_counter_instances_are_independent() {
        let a = RelaxedCounter::new();
        let b = RelaxedCounter::new();
        a.inc();
        a.inc();
        b.inc();
        assert_eq!(a.get(), 2);
        assert_eq!(b.get(), 1);
        a.inc();
        assert_eq!(a.get(), 3);
        assert_eq!(b.get(), 1);
    }
}
