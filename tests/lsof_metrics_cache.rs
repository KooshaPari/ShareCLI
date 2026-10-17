//! FR: FR-007 — Resource & Syscall-Relevant Watch
//!
//! Validates that lsof/ps process-metric samples are cached for <= 2 s per PID
//! with an injectable clock (no wall-clock / sleep assertions).
//!
//! AC references: AC-007.4 (FD watch), AC-007.8 (load average context),
//! plus the audit-task 1.12b acceptance criteria from PLAN.md.

use sharecli::runtime::count_open_fds;
use sharecli::runtime_cache::{CachedProcessMetrics, TestClock};
use std::sync::atomic::AtomicU64;

/// Second lookup inside TTL must hit cache — real fetcher invoked exactly once.
#[test]
fn second_lookup_within_ttl_hits_cache() {
    let now = AtomicU64::new(1_000);
    let clock = TestClock::new(&now);
    let cache = CachedProcessMetrics::with_clock(std::time::Duration::from_secs(2), clock);
    let pid = std::process::id();

    let first = cache.fetch_fd_count(pid, count_open_fds);
    assert!(first.is_some(), "fetch_fd_count must return Some for own pid");

    // Advance clock to 1 s — still within the 2 s TTL.
    now.store(2_000, std::sync::atomic::Ordering::SeqCst);
    let second = cache.fetch_fd_count(pid, count_open_fds);
    assert!(second.is_some());
    assert_eq!(first, second, "cached value must equal original");

    assert_eq!(cache.fetch_count(), 1, "fetcher must have been invoked exactly once");
}

/// Lookup past TTL must refetch — real fetcher invoked twice.
#[test]
fn lookup_past_ttl_refetches() {
    let now = AtomicU64::new(1_000);
    let clock = TestClock::new(&now);
    let cache = CachedProcessMetrics::with_clock(std::time::Duration::from_secs(2), clock);
    let pid = std::process::id();

    let first = cache.fetch_fd_count(pid, count_open_fds);
    assert!(first.is_some());

    // Advance past 2 s TTL.
    now.store(4_000, std::sync::atomic::Ordering::SeqCst);
    let second = cache.fetch_fd_count(pid, count_open_fds);
    assert!(second.is_some());

    assert_eq!(cache.fetch_count(), 2, "fetcher must have been invoked twice after TTL expiry");
}

/// Different PIDs must not share cache entries.
#[test]
fn per_pid_isolation() {
    let now = AtomicU64::new(0);
    let clock = TestClock::new(&now);
    let cache = CachedProcessMetrics::with_clock(std::time::Duration::from_secs(2), clock);

    let pid_a: u32 = 1;
    let pid_b: u32 = 2;

    let val_a = cache.fetch_thread_count(pid_a, |_| Some(10));
    let val_b = cache.fetch_thread_count(pid_b, |_| Some(20));

    assert_eq!(val_a, Some(10));
    assert_eq!(val_b, Some(20));
    assert_eq!(cache.fetch_count(), 2, "two different PIDs must each invoke the fetcher");

    // Re-fetch pid_a — should be cached.
    now.store(1_000, std::sync::atomic::Ordering::SeqCst);
    let val_a2 = cache.fetch_thread_count(pid_a, |_| Some(999));
    assert_eq!(val_a2, Some(10), "pid_a must still be cached (999 is stale)");
    assert_eq!(cache.fetch_count(), 2, "fetcher count must not increase for cached pid");
}
