//! Negative dentry cache — remember ENOENT lookups with TTL (FR-009).
//!
//! When a relative path is confirmed missing, subsequent probes within the TTL
//! return a cache hit without re-statting the backing filesystem. Create /
//! rename / mkdir into that name MUST [`NegativeDentryCache::invalidate`] the
//! entry so positive existence is visible immediately.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// Default negative-entry lifetime (matches InterceptFs FUSE entry TTL).
pub const DEFAULT_NEG_TTL: Duration = Duration::from_secs(1);

/// Default maximum number of negative entries retained (bounds cache memory).
pub const DEFAULT_NEG_CAP: usize = 65_536;

/// Idle window after which the sweeper drops an entry that was not re-probed.
pub const DEFAULT_NEG_SWEEP_IDLE: Duration = DEFAULT_NEG_TTL.saturating_mul(10);

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
    /// Last time this entry was probed (hit) — the sweeper's idle clock.
    last_probe: Instant,
    /// Monotonic recency sequence for LRU eviction ordering.
    recency: u64,
}

/// In-process negative dentry cache keyed by path relative to the backing root.
///
/// Bounded by a cap with LRU eviction; [`Self::sweep`] (also applied on cap
/// pressure) drops entries not re-probed within `DEFAULT_NEG_SWEEP_IDLE`.
#[derive(Debug)]
pub struct NegativeDentryCache {
    entries: HashMap<PathBuf, NegEntry>,
    /// Recency index: sequence -> path (min key = least recently probed).
    lru: BTreeMap<u64, PathBuf>,
    seq: u64,
    ttl: Duration,
    cap: usize,
    sweep_idle: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for NegativeDentryCache {
    fn default() -> Self {
        Self::with_ttl(DEFAULT_NEG_TTL)
    }
}

impl NegativeDentryCache {
    /// Empty cache with [`DEFAULT_NEG_TTL`] and [`DEFAULT_NEG_CAP`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty cache with an explicit TTL (tests / longer operator windows).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self::with_config(ttl, DEFAULT_NEG_CAP, DEFAULT_NEG_SWEEP_IDLE)
    }

    /// Empty cache with explicit TTL, entry cap, and sweeper idle window.
    ///
    /// `cap` is clamped to at least 1 so the cache always accepts an entry.
    pub fn with_config(ttl: Duration, cap: usize, sweep_idle: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            lru: BTreeMap::new(),
            seq: 0,
            ttl,
            cap: cap.max(1),
            sweep_idle,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Configured TTL for newly remembered misses.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Configured maximum number of retained entries.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Number of live negative entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when the cache holds no negative entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Current hit/miss counters.
    pub fn meters(&self) -> NegDentryMeters {
        NegDentryMeters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Refresh an entry's probe time + LRU recency.
    fn touch(&mut self, rel: &Path, now: Instant) {
        let seq = self.next_seq();
        let old_recency = match self.entries.get_mut(rel) {
            Some(entry) => {
                let old = entry.recency;
                entry.last_probe = now;
                entry.recency = seq;
                old
            }
            None => return,
        };
        self.lru.remove(&old_recency);
        self.lru.insert(seq, rel.to_path_buf());
    }

    fn remove_entry(&mut self, rel: &Path) -> bool {
        match self.entries.remove(rel) {
            Some(entry) => {
                self.lru.remove(&entry.recency);
                true
            }
            None => false,
        }
    }

    /// Evict the least-recently-probed entry; `false` when the cache is empty.
    fn evict_lru(&mut self) -> bool {
        match self.lru.pop_first() {
            Some((_, path)) => {
                self.entries.remove(&path);
                true
            }
            None => false,
        }
    }

    /// Drop entries not re-probed within `sweep_idle` as of `now`.
    ///
    /// Returns the number removed. Clock is injectable so tests stay
    /// deterministic (no sleeps).
    pub fn sweep_at(&mut self, now: Instant) -> usize {
        let stale: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(_, entry)| now.saturating_duration_since(entry.last_probe) > self.sweep_idle)
            .map(|(rel, _)| rel.clone())
            .collect();
        let removed = stale.len();
        for rel in stale {
            self.remove_entry(&rel);
        }
        removed
    }

    /// Drop entries not re-probed within the idle window (see [`Self::sweep_at`]).
    pub fn sweep(&mut self) -> usize {
        self.sweep_at(Instant::now())
    }

    /// Return `true` when `rel` is a still-valid negative entry (counts a hit).
    pub fn is_negative(&mut self, rel: &Path) -> bool {
        let now = Instant::now();
        match self.entries.get(rel) {
            Some(entry) if entry.expires_at > now => {
                self.touch(rel, now);
                self.hits.fetch_add(1, Ordering::Relaxed);
                true
            }
            Some(_) => {
                self.remove_entry(rel);
                false
            }
            None => false,
        }
    }

    /// Record that `rel` was confirmed missing (ENOENT) and count a miss.
    ///
    /// Bounded: stale entries are swept and, if still at cap, the
    /// least-recently-probed entry is evicted before the new insert.
    pub fn remember_miss(&mut self, rel: PathBuf) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now();
        if self.entries.contains_key(&rel) {
            if let Some(entry) = self.entries.get_mut(&rel) {
                entry.expires_at = now + self.ttl;
            }
            // Refresh probe time + recency (keeps the LRU index coherent).
            self.touch(&rel, now);
            return;
        }
        if self.entries.len() >= self.cap {
            self.sweep_at(now);
        }
        while self.entries.len() >= self.cap {
            if !self.evict_lru() {
                break;
            }
        }
        let seq = self.next_seq();
        self.lru.insert(seq, rel.clone());
        self.entries
            .insert(rel, NegEntry { expires_at: now + self.ttl, last_probe: now, recency: seq });
    }

    /// Drop a negative entry (e.g. after create / mkdir / rename-into).
    pub fn invalidate(&mut self, rel: &Path) {
        self.remove_entry(rel);
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

    // --- Task 1.17: cap + LRU + sweeper (FR-009) ---

    /// FR-009 — the cache never grows beyond its configured cap.
    #[test]
    fn neg_dentry_cap_bounds_entries() {
        let mut cache =
            NegativeDentryCache::with_config(Duration::from_secs(60), 2, Duration::from_secs(600));
        assert_eq!(cache.cap(), 2);
        cache.remember_miss(PathBuf::from("a"));
        cache.remember_miss(PathBuf::from("b"));
        cache.remember_miss(PathBuf::from("c"));
        assert_eq!(cache.len(), 2, "cap must bound the cache");
    }

    /// FR-009 — at cap, the least-recently-probed entry is evicted (a re-probed
    /// entry survives over an un-probed one).
    #[test]
    fn neg_dentry_lru_evicts_least_recently_probed() {
        let mut cache =
            NegativeDentryCache::with_config(Duration::from_secs(60), 3, Duration::from_secs(600));
        for name in ["a", "b", "c"] {
            cache.remember_miss(PathBuf::from(name));
        }
        // Re-probe "a" so it is the most recently used.
        assert!(cache.is_negative(Path::new("a")));
        cache.remember_miss(PathBuf::from("d"));
        assert_eq!(cache.len(), 3);
        assert!(cache.is_negative(Path::new("a")), "recently probed entry must survive");
        assert!(!cache.is_negative(Path::new("b")), "least-recently-probed entry must be evicted");
        assert!(cache.is_negative(Path::new("c")));
        assert!(cache.is_negative(Path::new("d")));
    }

    /// FR-009 — the sweeper drops entries not re-probed within the idle window
    /// while keeping recently probed ones (clock injected; no sleeps).
    #[test]
    fn neg_dentry_sweeper_drops_unprobed_entries() {
        let idle = Duration::from_secs(10);
        let mut cache = NegativeDentryCache::with_config(Duration::from_secs(600), 64, idle);
        cache.remember_miss(PathBuf::from("fresh"));
        cache.remember_miss(PathBuf::from("stale"));

        let now = Instant::now();
        cache.entries.get_mut(Path::new("stale")).expect("stale").last_probe =
            now - idle - Duration::from_secs(1);
        cache.entries.get_mut(Path::new("fresh")).expect("fresh").last_probe = now;

        assert_eq!(cache.sweep_at(now), 1, "only the un-re-probed entry is swept");
        assert!(cache.is_negative(Path::new("fresh")));
        assert!(!cache.is_negative(Path::new("stale")));
    }

    /// FR-009 — re-remembering an existing path refreshes its recency, so the
    /// LRU index stays coherent and a stale entry does not evict a live one.
    #[test]
    fn neg_dentry_remember_twice_refreshes_recency() {
        let mut cache =
            NegativeDentryCache::with_config(Duration::from_secs(600), 2, Duration::from_secs(600));
        cache.remember_miss(PathBuf::from("a"));
        cache.remember_miss(PathBuf::from("b"));
        cache.remember_miss(PathBuf::from("a")); // refresh "a"
        cache.remember_miss(PathBuf::from("c")); // must evict "b", not "a"
        assert_eq!(cache.len(), 2);
        assert!(cache.is_negative(Path::new("a")), "refreshed entry must not be evicted");
        assert!(!cache.is_negative(Path::new("b")), "un-refreshed entry is the LRU victim");
        assert!(cache.is_negative(Path::new("c")));
    }

    /// FR-009 — cap pressure sweeps stale entries before evicting live LRU ones.
    #[test]
    fn neg_dentry_cap_pressure_sweeps_stale_first() {
        let idle = Duration::from_secs(10);
        let mut cache = NegativeDentryCache::with_config(Duration::from_secs(600), 2, idle);
        cache.remember_miss(PathBuf::from("old1"));
        cache.remember_miss(PathBuf::from("old2"));

        let now = Instant::now();
        for name in ["old1", "old2"] {
            cache.entries.get_mut(Path::new(name)).expect(name).last_probe =
                now - idle - Duration::from_secs(1);
        }
        // Inserting at cap must reclaim the stale entries, not evict live ones.
        cache.remember_miss(PathBuf::from("new1"));
        assert_eq!(cache.len(), 1, "stale entries reclaimed on cap pressure");
        assert!(cache.is_negative(Path::new("new1")));
        assert!(!cache.is_negative(Path::new("old1")));
        assert!(!cache.is_negative(Path::new("old2")));
    }
}
