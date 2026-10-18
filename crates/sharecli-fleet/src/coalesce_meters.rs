// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Hypervisor coalesce cache operator meters (FR-008 / AC-008.11).

use std::sync::atomic::{AtomicU64, Ordering};

/// Snapshot of Hypervisor coalesce cache hit/miss counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct CoalesceMeters {
    /// Results served from cache (lookup, lock-recheck, debounce-recheck).
    pub hits: u64,
    /// Miss paths that executed the underlying command once.
    pub misses: u64,
    /// Mutating argv routed through the nocache queue bypass.
    pub nocache_runs: u64,
}

impl CoalesceMeters {
    /// Hit rate as an integer percentage in `[0, 100]` (0 when no hit/miss events).
    pub fn hit_rate_pct(self) -> u64 {
        let total = self.hits.saturating_add(self.misses);
        self.hits.saturating_mul(100).checked_div(total).unwrap_or(0)
    }

    /// Operator-facing status block for `sharecli status` (FR-008 / AC-008.11).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== Hypervisor Coalesce ===\n\n");
        out.push_str(&format!(
            "Cache hits:   {}\nCache misses: {}\nNocache runs: {}\nHit rate:     {}%\n",
            self.hits,
            self.misses,
            self.nocache_runs,
            self.hit_rate_pct()
        ));
        out
    }
}

static GLOBAL_COALESCE_HITS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_COALESCE_MISSES: AtomicU64 = AtomicU64::new(0);
static GLOBAL_NOCACHE_RUNS: AtomicU64 = AtomicU64::new(0);

/// Process-wide aggregate of Hypervisor coalesce events.
pub fn global_coalesce_meters() -> CoalesceMeters {
    CoalesceMeters {
        hits: GLOBAL_COALESCE_HITS.load(Ordering::Relaxed),
        misses: GLOBAL_COALESCE_MISSES.load(Ordering::Relaxed),
        nocache_runs: GLOBAL_NOCACHE_RUNS.load(Ordering::Relaxed),
    }
}

/// Record a pre-lock cache lookup hit (Hypervisor fast path before advisory flock).
pub fn record_coalesce_lookup_hit() {
    GLOBAL_COALESCE_HITS.fetch_add(1, Ordering::Relaxed);
}

/// Record a nocache queue execution (mutating argv bypass).
pub fn record_nocache_run() {
    GLOBAL_NOCACHE_RUNS.fetch_add(1, Ordering::Relaxed);
}

/// How a coalesce lock path obtained its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoalesceHitKind {
    /// The miss closure ran and stored a fresh entry.
    Miss,
    /// A sibling stored while this caller waited on the advisory flock.
    LockRecheck,
    /// A sibling stored during the debounce sleep before the miss closure ran.
    DebounceRecheck,
}

impl CoalesceHitKind {
    /// `true` when the result was served from cache without running the miss closure.
    pub fn shared_from_cache(self) -> bool {
        !matches!(self, Self::Miss)
    }
}

/// Record the outcome of a coalesce lock path.
pub fn record_coalesce_hit_kind(kind: CoalesceHitKind) {
    match kind {
        CoalesceHitKind::Miss => {
            GLOBAL_COALESCE_MISSES.fetch_add(1, Ordering::Relaxed);
        }
        CoalesceHitKind::LockRecheck | CoalesceHitKind::DebounceRecheck => {
            GLOBAL_COALESCE_HITS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_coalesce_meters_record_hit_miss_and_nocache() {
        let before = global_coalesce_meters();
        record_coalesce_lookup_hit();
        record_coalesce_hit_kind(CoalesceHitKind::Miss);
        record_coalesce_hit_kind(CoalesceHitKind::LockRecheck);
        record_nocache_run();
        let after = global_coalesce_meters();
        assert_eq!(after.hits, before.hits + 2);
        assert_eq!(after.misses, before.misses + 1);
        assert_eq!(after.nocache_runs, before.nocache_runs + 1);
        let section = after.format_status_section();
        assert!(section.contains("=== Hypervisor Coalesce ==="));
        assert!(section.contains("Nocache runs:"));
    }
}
