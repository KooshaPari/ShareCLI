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
            let cache = self.fd_cache.lock().unwrap();
            if let Some(v) = cache.get(&pid, now) {
                return v;
            }
        }
        // Cache miss — invoke the real producer.
        let value = fetcher(pid);
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let mut cache = self.fd_cache.lock().unwrap();
        cache.put(pid, value, now);
        value
    }

    /// Fetch thread count for `pid`, using `fetcher` on cache miss.
    pub fn fetch_thread_count(&self, pid: u32, fetcher: fn(u32) -> Option<u32>) -> Option<u32> {
        let now = self.clock.now_ms();
        {
            let cache = self.thread_cache.lock().unwrap();
            if let Some(v) = cache.get(&pid, now) {
                return v;
            }
        }
        let value = fetcher(pid);
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let mut cache = self.thread_cache.lock().unwrap();
        cache.put(pid, value, now);
        value
    }

    /// Total number of cache-miss fetches (both fd + thread).
    pub fn fetch_count(&self) -> u64 {
        self.fetch_count.load(Ordering::SeqCst)
    }
}
