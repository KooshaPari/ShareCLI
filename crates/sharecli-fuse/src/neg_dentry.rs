//! Negative dentry cache — remember ENOENT lookups with TTL (FR-009).
//!
//! When a relative path is confirmed missing, subsequent probes within the TTL
//! return a cache hit without re-statting the backing filesystem. Create /
//! rename / mkdir into that name MUST [`NegativeDentryCache::invalidate`] the
//! entry so positive existence is visible immediately.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// Default negative-entry lifetime (matches InterceptFs FUSE entry TTL).
pub const DEFAULT_NEG_TTL: Duration = Duration::from_secs(1);

/// Snapshot of negative-dentry meter counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NegDentryMeters {
    /// Lookups served from a still-valid negative entry (no backing stat).
    pub hits: u64,
    /// Lookups that recorded a fresh ENOENT into the cache.
    pub misses: u64,
}

impl NegDentryMeters {
    /// Hit rate as an integer percentage in `[0, 100]` (0 when no events recorded).
    pub fn hit_rate_pct(self) -> u64 {
        let total = self.hits.saturating_add(self.misses);
        self.hits.saturating_mul(100).checked_div(total).unwrap_or(0)
    }

    /// Operator-facing status block for `sharecli status` (FR-009 / AC-009.9).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== FUSE Negative Dentry ===\n\n");
        out.push_str(&format!(
            "Neg hits:     {}\nNeg misses:   {}\nHit rate:     {}%\n",
            self.hits,
            self.misses,
            self.hit_rate_pct()
        ));
        out
    }
}

static GLOBAL_NEG_HITS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_NEG_MISSES: AtomicU64 = AtomicU64::new(0);

/// Record a process-wide negative-dentry hit (InterceptFs boundary only).
pub(crate) fn record_global_neg_hit() {
    GLOBAL_NEG_HITS.fetch_add(1, Ordering::Relaxed);
}

/// Record a process-wide negative-dentry miss (InterceptFs boundary only).
pub(crate) fn record_global_neg_miss() {
    GLOBAL_NEG_MISSES.fetch_add(1, Ordering::Relaxed);
}

/// Process-wide aggregate of negative-dentry hit/miss events across all FUSE intercepts.
pub fn global_neg_dentry_meters() -> NegDentryMeters {
    NegDentryMeters {
        hits: GLOBAL_NEG_HITS.load(Ordering::Relaxed),
        misses: GLOBAL_NEG_MISSES.load(Ordering::Relaxed),
    }
}

#[derive(Debug, Clone)]
struct NegEntry {
    expires_at: Instant,
}

/// In-process negative dentry cache keyed by path relative to the backing root.
#[derive(Debug)]
pub struct NegativeDentryCache {
    entries: HashMap<PathBuf, NegEntry>,
    ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for NegativeDentryCache {
    fn default() -> Self {
        Self::with_ttl(DEFAULT_NEG_TTL)
    }
}

impl NegativeDentryCache {
    /// Empty cache with [`DEFAULT_NEG_TTL`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty cache with an explicit TTL (tests / longer operator windows).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { entries: HashMap::new(), ttl, hits: AtomicU64::new(0), misses: AtomicU64::new(0) }
    }

    /// Configured TTL for newly remembered misses.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Current hit/miss counters.
    pub fn meters(&self) -> NegDentryMeters {
        NegDentryMeters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    /// Return `true` when `rel` is a still-valid negative entry (counts a hit).
    pub fn is_negative(&mut self, rel: &Path) -> bool {
        let now = Instant::now();
        match self.entries.get(rel) {
            Some(entry) if entry.expires_at > now => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                true
            }
            Some(_) => {
                self.entries.remove(rel);
                false
            }
            None => false,
        }
    }

    /// Record that `rel` was confirmed missing (ENOENT) and count a miss.
    pub fn remember_miss(&mut self, rel: PathBuf) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.entries.insert(rel, NegEntry { expires_at: Instant::now() + self.ttl });
    }

    /// Drop a negative entry (e.g. after create / mkdir / rename-into).
    pub fn invalidate(&mut self, rel: &Path) {
        self.entries.remove(rel);
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    /// FR-009 / AC-009.7 — remember miss then hit within TTL.
    #[test]
    #[serial_test::serial]
    fn neg_dentry_miss_then_hit() {
        let mut cache = NegativeDentryCache::with_ttl(Duration::from_secs(30));
        let rel = PathBuf::from("missing.txt");
        assert!(!cache.is_negative(&rel));
        cache.remember_miss(rel.clone());
        assert!(cache.is_negative(&rel));
        let m = cache.meters();
        assert_eq!(m.misses, 1);
        assert_eq!(m.hits, 1);
    }

    /// FR-009 / AC-009.7 — invalidate clears the negative entry.
    #[test]
    fn neg_dentry_invalidate_clears() {
        let mut cache = NegativeDentryCache::with_ttl(Duration::from_secs(30));
        let rel = PathBuf::from("gone.txt");
        cache.remember_miss(rel.clone());
        cache.invalidate(&rel);
        assert!(!cache.is_negative(&rel));
        assert_eq!(cache.meters().hits, 0);
    }

    /// FR-009 / AC-009.7 — expired entries are dropped (no hit).
    #[test]
    fn neg_dentry_ttl_expiry() {
        let mut cache = NegativeDentryCache::with_ttl(Duration::from_millis(20));
        let rel = PathBuf::from("stale.txt");
        cache.remember_miss(rel.clone());
        thread::sleep(Duration::from_millis(40));
        assert!(!cache.is_negative(&rel));
        assert_eq!(cache.meters().hits, 0);
        assert_eq!(cache.meters().misses, 1);
    }

    #[test]
    fn neg_dentry_format_status_section() {
        let section = NegDentryMeters { hits: 2, misses: 1 }.format_status_section();
        assert!(section.contains("=== FUSE Negative Dentry ==="));
        assert!(section.contains("Neg hits:"));
        assert!(section.contains("Hit rate:     66%"));
    }

    // --- Additional coverage tests ---

    #[test]
    fn neg_dentry_default_ttl() {
        let mut cache = NegativeDentryCache::new();
        let rel = PathBuf::from("tmp.txt");
        cache.remember_miss(rel.clone());
        assert!(cache.is_negative(&rel));
    }

    #[test]
    fn neg_dentry_multiple_entries() {
        let mut cache = NegativeDentryCache::new();
        cache.remember_miss(PathBuf::from("a.txt"));
        cache.remember_miss(PathBuf::from("b.txt"));
        cache.remember_miss(PathBuf::from("c.txt"));
        assert!(cache.is_negative(&PathBuf::from("a.txt")));
        assert!(cache.is_negative(&PathBuf::from("b.txt")));
        assert!(cache.is_negative(&PathBuf::from("c.txt")));
    }

    #[test]
    fn neg_dentry_meters_zero() {
        let m = NegDentryMeters { hits: 0, misses: 0 };
        let s = m.format_status_section();
        assert!(s.contains("Neg hits:     0"));
        assert!(s.contains("Neg misses:   0"));
        assert!(s.contains("0%"));
    }

    #[test]
    fn neg_dentry_invalidate_removes() {
        let mut cache = NegativeDentryCache::new();
        let rel = PathBuf::from("invalidate_me.txt");
        cache.remember_miss(rel.clone());
        assert!(cache.is_negative(&rel));
        cache.invalidate(&rel);
        assert!(!cache.is_negative(&rel));
    }

    #[test]
    fn neg_dentry_meters_100_percent() {
        let m = NegDentryMeters { hits: 5, misses: 0 };
        let s = m.format_status_section();
        assert!(s.contains("100%"));
    }

    #[test]
    fn neg_dentry_meters_zero_percent() {
        let m = NegDentryMeters { hits: 0, misses: 10 };
        let s = m.format_status_section();
        assert!(s.contains("0%"));
    }
}
