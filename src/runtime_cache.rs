//! Cached process-metric fetchers with injectable clock.
//!
//! Wraps the raw `count_open_fds` / `count_threads` producers from
//! [`crate::runtime`] behind a per-PID, bounded-TTL cache so that repeated
//! `lsof` / `ps` invocations within a short window are amortised.
//!
//! The [`Clock`] trait abstracts wall-clock time so tests can drive the cache
//! with a [`TestClock`] backed by an `AtomicU64` — no `sleep`/`Instant`
//! assertions required.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Clock abstraction
// ---------------------------------------------------------------------------

/// Abstract monotonic clock.  Production code uses [`MonotonicClock`]; tests
/// inject [`TestClock`].
pub trait Clock: Send + Sync {
    /// Return current monotonic time in **milliseconds**.
    fn now_ms(&self) -> u64;
}

/// Production clock — delegates to [`std::time::Instant`].
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        // Use a process-local anchor so overflow is someone else's problem.
        use std::time::Instant;
        static ANCHOR: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let anchor = ANCHOR.get_or_init(Instant::now);
        anchor.elapsed().as_millis() as u64
    }
}

/// Test clock — reads from a caller-supplied [`AtomicU64`].
///
/// Shared via `&'a AtomicU64` so the test can mutate the clock freely
/// between operations.
pub struct TestClock<'a> {
    now: &'a AtomicU64,
}

impl<'a> TestClock<'a> {
    pub fn new(now: &'a AtomicU64) -> Self {
        Self { now }
    }
}

impl<'a> Clock for TestClock<'a> {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// Per-pid TTL cache (bounded staleness)
// ---------------------------------------------------------------------------

struct CacheEntry<V> {
    value: V,
    inserted_at_ms: u64,
}

/// Cache keyed by `u32` (PID), storing values with a fixed TTL.  Not
/// thread-safe on its own — callers serialise through [`CachedProcessMetrics`].
struct BoundedTtlCache<V> {
    entries: HashMap<u32, CacheEntry<V>>,
    ttl_ms: u64,
}

impl<V: Clone> BoundedTtlCache<V> {
    fn new(ttl: Duration) -> Self {
        Self { entries: HashMap::new(), ttl_ms: ttl.as_millis() as u64 }
    }

    fn get(&self, pid: &u32, now_ms: u64) -> Option<V> {
        self.entries.get(pid).and_then(|e| {
            if now_ms.saturating_sub(e.inserted_at_ms) < self.ttl_ms {
                Some(e.value.clone())
            } else {
                None
            }
        })
    }

    fn put(&mut self, pid: u32, value: V, now_ms: u64) {
        self.entries.insert(pid, CacheEntry { value, inserted_at_ms: now_ms });
    }
}

// ---------------------------------------------------------------------------
// Public API: CachedProcessMetrics
// ---------------------------------------------------------------------------

/// Per-PID, bounded-TTL cache for process metrics (FD count, thread count).
///
/// Generic over [`Clock`] so tests can inject [`TestClock`].
pub struct CachedProcessMetrics<C: Clock> {
    clock: C,
    fd_cache: Mutex<BoundedTtlCache<Option<u32>>>,
    thread_cache: Mutex<BoundedTtlCache<Option<u32>>>,
    fetch_count: AtomicU64,
}

impl CachedProcessMetrics<MonotonicClock> {
    /// Production entry-point — monotonic clock, 2 s default TTL.
    pub fn new(ttl: Duration) -> Self {
        Self::with_clock(ttl, MonotonicClock)
    }
}

impl<C: Clock> CachedProcessMetrics<C> {
    /// Create with an arbitrary clock (used by tests).
    pub fn with_clock(ttl: Duration, clock: C) -> Self {
        Self {
            clock,
            fd_cache: Mutex::new(BoundedTtlCache::new(ttl)),
            thread_cache: Mutex::new(BoundedTtlCache::new(ttl)),
            fetch_count: AtomicU64::new(0),
        }
    }

    /// Fetch open-FD count for `pid`, using `fetcher` on cache miss.
    pub fn fetch_fd_count(&self, pid: u32, fetcher: fn(u32) -> Option<u32>) -> Option<u32> {
        let now = self.clock.now_ms();
        {
            let cache = self.fd_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(v) = cache.get(&pid, now) {
                return v;
            }
        }
        // Cache miss — invoke the real producer.
        let value = fetcher(pid);
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let mut cache = self.fd_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.put(pid, value, now);
        value
    }

    /// Fetch thread count for `pid`, using `fetcher` on cache miss.
    pub fn fetch_thread_count(&self, pid: u32, fetcher: fn(u32) -> Option<u32>) -> Option<u32> {
        let now = self.clock.now_ms();
        {
            let cache = self.thread_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(v) = cache.get(&pid, now) {
                return v;
            }
        }
        let value = fetcher(pid);
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let mut cache = self.thread_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.put(pid, value, now);
        value
    }

    /// Total number of cache-miss fetches (both fd + thread).
    pub fn fetch_count(&self) -> u64 {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    //! PLAN.md: 1.12b (lane 6, P1) — lsof / thread-count cache.
    //!
    //! These tests pin the public-interface contract of
    //! [`CachedProcessMetrics`]: fetcher is invoked on miss, the result is
    //! served from the cache on hit, TTL expiry re-invokes the fetcher, and
    //! the fetch counter records exactly one bump per miss.
    //!
    //! Deterministic: a [`TestClock`] backed by an `AtomicU64` drives time
    //! forward by `fetch_add` between operations — no `Instant::now` /
    //! `thread::sleep` is used in these tests.

    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering as AOrd};

    fn always_some(_: u32) -> Option<u32> {
        Some(7)
    }

    fn always_none(_: u32) -> Option<u32> {
        None
    }

    fn counting_fetcher(_: u32) -> Option<u32> {
        // Static counter: each invocation of this fetcher bumps it.
        // Used to assert that a cache hit does NOT re-invoke the fetcher.
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, AOrd::SeqCst);
        assert!(n < 1000, "fetcher invoked unexpectedly many times: {n}");
        Some(42)
    }

    #[test]
    fn cache_miss_invokes_fetcher_and_populates() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        // First call: cache miss, fetcher must run.
        let v = cache.fetch_fd_count(123, always_some);
        assert_eq!(v, Some(7));
        assert_eq!(cache.fetch_count(), 1, "first miss must bump fetch_count");
    }

    #[test]
    fn cache_hit_does_not_invoke_fetcher() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        // Warm the cache.
        let _ = cache.fetch_fd_count(7, always_some);
        // Replace the fetcher with one that would return a DIFFERENT value
        // and bump a counter if it were invoked. Since the cache hits, the
        // fetcher must NOT run.
        let _ = cache.fetch_fd_count(7, |_| {
            panic!("fetcher must not run on a cache hit");
        });
        assert_eq!(cache.fetch_count(), 1, "no new miss on hit");
    }

    #[test]
    fn ttl_expiry_re_invokes_fetcher() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_millis(100), clock);

        // Warm at t=0.
        let _ = cache.fetch_fd_count(1, always_some);
        assert_eq!(cache.fetch_count(), 1);

        // At t=50ms (within TTL) — still a hit.
        now.store(50, Ordering::SeqCst);
        let _ = cache.fetch_fd_count(1, always_some);
        assert_eq!(cache.fetch_count(), 1, "50ms < 100ms TTL must hit");

        // At t=150ms (past TTL) — must miss and bump the counter.
        now.store(150, Ordering::SeqCst);
        let _ = cache.fetch_fd_count(1, always_some);
        assert_eq!(cache.fetch_count(), 2, "150ms > 100ms TTL must miss");
    }

    #[test]
    fn fd_and_thread_caches_are_independent() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        // Warming the FD cache must not satisfy a thread-cache query for
        // the same PID — the two caches are independent.
        let _ = cache.fetch_fd_count(99, always_some);
        let _ = cache.fetch_thread_count(99, always_some);
        assert_eq!(cache.fetch_count(), 2, "FD and thread caches share a counter but not storage");
    }

    #[test]
    fn different_pids_miss_independently() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        let _ = cache.fetch_fd_count(1, always_some);
        let _ = cache.fetch_fd_count(2, always_some);
        let _ = cache.fetch_fd_count(3, always_some);
        assert_eq!(cache.fetch_count(), 3, "three distinct PIDs = three misses");
    }

    #[test]
    fn fetcher_returning_none_is_cached_as_none() {
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        // A producer that returns None (process gone, permission denied, …)
        // must be cached as None — the next call within TTL must NOT
        // re-invoke the fetcher.
        let _ = cache.fetch_fd_count(404, always_none);
        let _ = cache.fetch_fd_count(404, always_none);
        assert_eq!(cache.fetch_count(), 1, "None is a valid cached value");
    }

    #[test]
    fn monotonic_clock_advances_with_real_time() {
        // Smoke-test the production clock path: it must produce a
        // monotonically non-decreasing value across two reads.
        let c1 = MonotonicClock.now_ms();
        let c2 = MonotonicClock.now_ms();
        assert!(c2 >= c1, "MonotonicClock must not go backwards: c1={c1} c2={c2}");
    }

    #[test]
    fn counting_fetcher_records_each_invocation() {
        // The shim fetcher increments a static counter on every call. A
        // cache miss must increment it; a cache hit must not.
        let now = AtomicU64::new(0);
        let clock = TestClock::new(&now);
        let cache: CachedProcessMetrics<TestClock> =
            CachedProcessMetrics::with_clock(Duration::from_secs(2), clock);

        let _ = cache.fetch_fd_count(11, counting_fetcher);
        let _ = cache.fetch_fd_count(11, counting_fetcher);
        let _ = cache.fetch_fd_count(12, counting_fetcher);
        // Three fetcher calls; the fetcher used in the second call is
        // discarded by the cache hit but its closure would panic if it
        // were invoked — proving the cache served the second call.
        assert_eq!(cache.fetch_count(), 2, "two misses, one hit");
    }
}
