//! `sharecli-ipc` — coalesce/debounce/queue tier of the sharecli OS-process hypervisor.
//!
//! # Lock-Wait-Cache pattern
//!
//! When N isolated agent processes issue the same command concurrently (e.g. 8 agents
//! all run `ruff check .`), only one execution actually runs; the other 7 block on an
//! advisory `flock` and then read the result written by the winner.
//!
//! The building blocks are:
//!
//! 1. **[`command_key`]** — SHA-256 of (argv + cwd + relevant env) → deterministic hex key.
//! 2. **[`CoalesceCache`]** — atomic JSON store: `lookup` / `store` / `with_lock`.
//! 3. **[`CachedResult`]** — the serialisable exit_code + stdout + stderr bundle.
//! 4. **[`SlotQueue`] / [`PriorityQueue`]** — N-slot concurrency limiter for mutating paths.
//! 5. **[`has_nocache_arg`]** — Feb `nocache_args` detection (coalesce → queue fallback).
//!
//! # TTL + debounce (origin harness coalesce)
//!
//! - **TTL** — `lookup` treats entries whose mtime age is ≥ configured TTL as a miss
//!   (default [`CoalesceCache::DEFAULT_TTL`] = 300s). `store` sweeps stale `*.json`
//!   entries under the cache root.
//! - **Debounce** — on a miss, `with_lock` waits `debounce` then re-checks the cache so
//!   a concurrent store completed in-window is shared instead of re-running (origin
//!   harness `debounce_ms`; default off).
//!
//! # Queue + nocache (Feb harness FR-008)
//!
//! Mutating argv (`--fix`, `--force`, `--write`, …) MUST NOT hit [`CoalesceCache`].
//! Use [`should_bypass_coalesce`] then [`SlotQueue::with_slot`]. Hypervisor callers:
//! see `sharecli_core::Hypervisor::{queue, run}`.

pub mod cache_key;
pub mod handler;
pub mod log_buffer;
pub mod nocache;
pub mod queue;
pub mod semantic;
pub mod serve_lock;
pub mod ws_client;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
pub use cache_key::{command_key, command_key_with_mode, CacheKeyMode};
use fs2::FileExt;
pub use nocache::{
    has_nocache_arg, parse_nocache_args_csv, should_bypass_coalesce, DEFAULT_NOCACHE_ARGS,
};
pub use queue::{
    resolve_operator_queue_priority, PriorityQueue, QueuePriority, SlotQueue, QUEUE_PRIORITY_ENV,
};
pub use semantic::semantic_normalize_argv;
use serde::{Deserialize, Serialize};
pub use sharecli_fleet::{
    global_coalesce_meters, global_slot_queue_meters, record_coalesce_hit_kind,
    record_coalesce_lookup_hit, record_nocache_run, record_slot_acquire, record_slot_timeout,
    record_slot_wait, CoalesceHitKind, CoalesceMeters, SlotQueueMeters,
};

// ---------------------------------------------------------------------------
// CommandKey
// ---------------------------------------------------------------------------

/// An opaque, stable, hex-encoded cache key for a command invocation.
///
/// See [`command_key_with_mode`] and [`CacheKeyMode`] for dimension rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandKey(pub String);

// ---------------------------------------------------------------------------
// CachedResult
// ---------------------------------------------------------------------------

/// The outcome of a command execution — what the hypervisor stores and returns
/// to the waiting sibling agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    pub exit_code: i32,
    /// Raw bytes of standard output.
    pub stdout: Vec<u8>,
    /// Raw bytes of standard error.
    pub stderr: Vec<u8>,
}

// ---------------------------------------------------------------------------
// CoalesceCache
// ---------------------------------------------------------------------------

/// File-system-backed coalesce cache for command results.
///
/// Layout under `root/`:
/// ```text
/// <hex-key>.json   — JSON-serialised CachedResult
/// <hex-key>.lock   — advisory flock sentinel (content irrelevant)
/// ```
///
/// [`with_lock`][CoalesceCache::with_lock] serialises concurrent callers with the
/// same key: the first acquires the exclusive flock, runs the command, and writes the
/// result; subsequent callers block until the lock is released, then hit the now-
/// populated cache entry without re-executing.
///
/// Entries older than [`ttl`][CoalesceCache::ttl] are treated as misses. When
/// [`debounce`][CoalesceCache::debounce] is non-zero, a miss path sleeps then
/// re-checks so an in-window sibling store is shared (origin harness `debounce_ms`).
pub struct CoalesceCache {
    root: PathBuf,
    ttl: Duration,
    debounce: Duration,
}

impl Clone for CoalesceCache {
    fn clone(&self) -> Self {
        Self { root: self.root.clone(), ttl: self.ttl, debounce: self.debounce }
    }
}

impl CoalesceCache {
    /// Default result lifetime (5 minutes) — longer than origin harness lint TTLs
    /// so Hypervisor callers share across agent turns unless overridden.
    pub const DEFAULT_TTL: Duration = Duration::from_secs(300);

    /// Create a new cache rooted at `root` with [`DEFAULT_TTL`][Self::DEFAULT_TTL]
    /// and debounce disabled. The directory is created on first use if needed.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_options(root, Self::DEFAULT_TTL, Duration::ZERO)
    }

    /// Create a cache with a custom TTL and debounce disabled.
    pub fn with_ttl(root: impl Into<PathBuf>, ttl: Duration) -> Self {
        Self::with_options(root, ttl, Duration::ZERO)
    }

    /// Create a cache with explicit TTL and debounce window.
    ///
    /// `ttl` — max age of a stored entry before `lookup` returns a miss.
    /// `debounce` — on miss, wait this long then re-check before running the
    /// miss path (origin harness `debounce_ms`; `Duration::ZERO` disables).
    pub fn with_options(root: impl Into<PathBuf>, ttl: Duration, debounce: Duration) -> Self {
        Self { root: root.into(), ttl, debounce }
    }

    /// Configured TTL for cache entries.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Configured debounce window (zero = disabled).
    pub fn debounce(&self) -> Duration {
        self.debounce
    }

    fn entry_path(&self, key: &CommandKey) -> PathBuf {
        self.root.join(format!("{}.json", key.0))
    }

    fn lock_path(&self, key: &CommandKey) -> PathBuf {
        self.root.join(format!("{}.lock", key.0))
    }

    fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create cache root {}", self.root.display()))
    }

    /// Age of `path` from mtime, or `None` if the file is missing.
    fn entry_age(&self, path: &Path) -> Result<Option<Duration>> {
        match fs::metadata(path) {
            Ok(meta) => {
                let modified =
                    meta.modified().with_context(|| format!("mtime for {}", path.display()))?;
                let age = SystemTime::now().duration_since(modified).unwrap_or(Duration::ZERO);
                Ok(Some(age))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("metadata for {}", path.display())),
        }
    }

    fn is_fresh(&self, path: &Path) -> Result<bool> {
        match self.entry_age(path)? {
            Some(age) => Ok(age < self.ttl),
            None => Ok(false),
        }
    }

    /// Remove `*.json` entries under `root` whose mtime exceeds TTL.
    fn evict_stale(&self) -> Result<()> {
        let entries = match fs::read_dir(&self.root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(e).with_context(|| format!("read cache root {}", self.root.display()))
            }
        };

        for entry in entries {
            let entry =
                entry.with_context(|| format!("read dir entry in {}", self.root.display()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if !self.is_fresh(&path)? {
                let _ = fs::remove_file(&path);
            }
        }
        Ok(())
    }

    /// Look up a cached result.
    ///
    /// Returns `Ok(None)` when no entry exists, or when the entry's mtime age is
    /// ≥ the configured TTL (stale → miss).
    pub fn lookup(&self, key: &CommandKey) -> Result<Option<CachedResult>> {
        let path = self.entry_path(key);
        if !self.is_fresh(&path)? {
            // Stale or missing — treat as miss (leave file for eviction sweep).
            return Ok(None);
        }
        match fs::read(&path) {
            Ok(bytes) => {
                let result: CachedResult = serde_json::from_slice(&bytes)
                    .with_context(|| format!("deserialise cache entry {}", path.display()))?;
                Ok(Some(result))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("read cache entry {}", path.display())),
        }
    }

    /// Atomically write a [`CachedResult`] for `key`.
    ///
    /// Uses a write-to-temp-then-rename strategy so concurrent readers never
    /// observe a partial / truncated JSON file. After writing, sweeps stale
    /// entries under `root/` whose mtime exceeds the configured TTL.
    pub fn store(&self, key: &CommandKey, result: &CachedResult) -> Result<()> {
        self.ensure_root()?;

        let bytes = serde_json::to_vec(result).context("serialise CachedResult")?;

        // Write to a NamedTempFile in the same directory so the rename is atomic
        // (same filesystem, no cross-device move).
        let mut tmp = tempfile::NamedTempFile::new_in(&self.root)
            .with_context(|| format!("create temp file in {}", self.root.display()))?;
        tmp.write_all(&bytes).context("write cache bytes to temp file")?;
        tmp.flush().context("flush temp file")?;

        let dest = self.entry_path(key);
        tmp.persist(&dest).with_context(|| format!("persist cache entry to {}", dest.display()))?;

        self.evict_stale()?;

        Ok(())
    }

    /// Execute `f` under an exclusive advisory flock for `key`.
    ///
    /// The Lock-Wait-Cache protocol:
    /// 1. Open (or create) `root/<key>.lock`.
    /// 2. Acquire an **exclusive** flock — blocks until any prior holder releases it.
    /// 3. After acquiring the lock, **re-check** the cache (TTL-aware): a sibling that
    ///    held the lock may have already stored the result.
    /// 4. On miss with debounce configured: sleep, then re-check so an in-window
    ///    store from another process is shared instead of re-running.
    /// 5. If still a miss, call `f()` and store the result.
    /// 6. Release the lock (file handle drop).
    ///
    /// Returns the [`CachedResult`] whether it came from `f()` or the cache.
    pub fn with_lock<T>(&self, key: &CommandKey, f: impl FnOnce() -> Result<T>) -> Result<T>
    where
        T: Into<CachedResult> + From<CachedResult>,
    {
        self.with_lock_detailed(key, f).map(|(value, _)| value)
    }

    /// Like [`with_lock`][Self::with_lock] but reports whether the result came from
    /// a lock/debounce re-check instead of the miss closure.
    pub fn with_lock_detailed<T>(
        &self,
        key: &CommandKey,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<(T, CoalesceHitKind)>
    where
        T: Into<CachedResult> + From<CachedResult>,
    {
        self.ensure_root()?;

        let lock_path = self.lock_path(key);
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open lock file {}", lock_path.display()))?;

        // Block until we are the sole holder.
        lock_file
            .lock_exclusive()
            .with_context(|| format!("acquire exclusive lock on {}", lock_path.display()))?;

        // Re-check: a sibling may have stored the result while we were waiting.
        if let Some(cached) = self.lookup(key)? {
            record_coalesce_hit_kind(CoalesceHitKind::LockRecheck);
            return Ok((T::from(cached), CoalesceHitKind::LockRecheck));
        }

        // Debounce window (origin harness coalesce debounce_ms): wait, then share
        // if another process stored a fresh result while we held the miss.
        if !self.debounce.is_zero() {
            thread::sleep(self.debounce);
            if let Some(cached) = self.lookup(key)? {
                record_coalesce_hit_kind(CoalesceHitKind::DebounceRecheck);
                return Ok((T::from(cached), CoalesceHitKind::DebounceRecheck));
            }
        }

        // We are first — run the command.
        let value = f()?;
        let cached: CachedResult = value.into();

        // Lane-7 BLOCKER (tiger): failed runs were stored and replayed to siblings
        // for the full TTL. Skip persistence for any non-success result so siblings
        // re-run on their own; success results are stored as before.
        if cached.exit_code != 0 {
            record_coalesce_hit_kind(CoalesceHitKind::Miss);
            return Ok((T::from(cached), CoalesceHitKind::Miss));
        }

        self.store(key, &cached)?;
        record_coalesce_hit_kind(CoalesceHitKind::Miss);

        // Lock releases on drop.
        Ok((T::from(cached), CoalesceHitKind::Miss))
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    // -----------------------------------------------------------------------
    // (a) command_key is stable and differs on differing argv
    // -----------------------------------------------------------------------
    #[test]
    fn command_key_stable_and_differs() {
        let cwd = Path::new("/tmp/proj");
        let env: Vec<(String, String)> = vec![];

        let argv_a = vec!["ruff".to_string(), "check".to_string(), ".".to_string()];
        let argv_b = vec!["ruff".to_string(), "format".to_string(), ".".to_string()];

        let key1 = command_key(&argv_a, cwd, &env);
        let key2 = command_key(&argv_a, cwd, &env);
        let key3 = command_key(&argv_b, cwd, &env);

        // Stable: same input → same key.
        assert_eq!(key1, key2, "command_key must be deterministic");

        // Differs: different argv → different key.
        assert_ne!(key1, key3, "different argv must produce different keys");

        // Key is a non-empty hex string (64 hex chars for SHA-256).
        assert_eq!(key1.0.len(), 64);
        assert!(key1.0.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // -----------------------------------------------------------------------
    // (b) store → lookup round-trips a CachedResult
    // -----------------------------------------------------------------------
    #[test]
    fn store_lookup_round_trip() {
        let dir = TempDir::new().expect("tempdir");
        let cache = CoalesceCache::new(dir.path());

        let argv = vec!["cargo".to_string(), "check".to_string()];
        let key = command_key(&argv, Path::new("/workspace"), &[]);

        // Nothing stored yet.
        assert!(cache.lookup(&key).unwrap().is_none(), "fresh cache should be empty");

        let result = CachedResult { exit_code: 0, stdout: b"all good".to_vec(), stderr: vec![] };

        cache.store(&key, &result).expect("store");

        let got = cache.lookup(&key).expect("lookup").expect("should be Some");
        assert_eq!(got.exit_code, 0);
        assert_eq!(got.stdout, b"all good");
        assert_eq!(got.stderr, Vec::<u8>::new());
    }

    // -----------------------------------------------------------------------
    // (c) with_lock: second call returns cached result without re-running f
    //     (only true for successful runs; failed runs are not cached.)
    // -----------------------------------------------------------------------
    #[test]
    fn with_lock_deduplicates() {
        let dir = TempDir::new().expect("tempdir");
        let cache = CoalesceCache::new(dir.path());

        let argv = vec!["pytest".to_string(), "-x".to_string()];
        let key = command_key(&argv, Path::new("/repo"), &[]);

        let mut call_count = 0u32;

        // First call — f() should execute.
        let r1: CachedResult = cache
            .with_lock(&key, || {
                call_count += 1;
                Ok(CachedResult { exit_code: 0, stdout: b"run1".to_vec(), stderr: vec![] })
            })
            .expect("first with_lock");
        assert_eq!(call_count, 1, "f() must run on first call");
        assert_eq!(r1.exit_code, 0);

        // Second call — cache is populated, f() must NOT run again.
        let r2: CachedResult = cache
            .with_lock(&key, || {
                call_count += 1;
                Ok(CachedResult { exit_code: 99, stdout: b"run2".to_vec(), stderr: vec![] })
            })
            .expect("second with_lock");
        assert_eq!(call_count, 1, "f() must NOT run when cache is populated");
        assert_eq!(r2.exit_code, 0, "second call must return the cached result");
        assert_eq!(r2.stdout, b"run1");
    }

    // -----------------------------------------------------------------------
    // FR-008 / AC-008.5 — TTL stale miss + store eviction
    // -----------------------------------------------------------------------
    #[test]
    fn ttl_lookup_miss_and_evict_on_store() {
        let dir = TempDir::new().expect("tempdir");
        let ttl = Duration::from_millis(60);
        let cache = CoalesceCache::with_ttl(dir.path(), ttl);
        assert_eq!(cache.ttl(), ttl);
        assert_eq!(cache.debounce(), Duration::ZERO);

        let stale_key = command_key(&["stale".into()], Path::new("/x"), &[]);
        let fresh_key = command_key(&["fresh".into()], Path::new("/x"), &[]);

        cache
            .store(
                &stale_key,
                &CachedResult { exit_code: 0, stdout: b"old".to_vec(), stderr: vec![] },
            )
            .expect("store stale candidate");

        thread::sleep(ttl + Duration::from_millis(30));
        assert!(cache.lookup(&stale_key).unwrap().is_none());

        cache
            .store(
                &fresh_key,
                &CachedResult { exit_code: 0, stdout: b"new".to_vec(), stderr: vec![] },
            )
            .expect("store triggers eviction");

        assert!(
            !cache.entry_path(&stale_key).exists(),
            "store MUST evict TTL-expired json entries"
        );
        assert!(cache.lookup(&fresh_key).unwrap().is_some());
    }

    // -----------------------------------------------------------------------
    // FR-008 / AC-008.6 — debounce re-check shares in-window store
    // -----------------------------------------------------------------------
    #[test]
    fn debounce_shares_recent_store() {
        let dir = TempDir::new().expect("tempdir");
        let debounce = Duration::from_millis(100);
        let cache = CoalesceCache::with_options(dir.path(), Duration::from_secs(60), debounce);
        let key = command_key(&["deb".into()], Path::new("/y"), &[]);

        let root = dir.path().to_path_buf();
        let key_bg = key.clone();
        let producer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            let bg = CoalesceCache::with_options(&root, Duration::from_secs(60), Duration::ZERO);
            bg.store(
                &key_bg,
                &CachedResult { exit_code: 0, stdout: b"from-bg".to_vec(), stderr: vec![] },
            )
            .expect("bg store");
        });

        let mut ran = false;
        let (got, kind) = cache
            .with_lock_detailed(&key, || {
                ran = true;
                Ok(CachedResult { exit_code: 7, stdout: b"miss".to_vec(), stderr: vec![] })
            })
            .expect("with_lock_detailed");

        producer.join().expect("join");
        assert!(!ran, "debounce MUST share bg store without miss path");
        assert_eq!(got.stdout, b"from-bg");
        assert_eq!(kind, CoalesceHitKind::DebounceRecheck);
    }

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

    // -----------------------------------------------------------------------
    // Lane-7 BLOCKER (tiger): failed runs MUST NOT be cached, otherwise the
    // next sibling sees the broken result for the full TTL.
    // -----------------------------------------------------------------------
    #[test]
    fn with_lock_does_not_cache_failed_run() {
        let dir = TempDir::new().expect("tempdir");
        let cache = CoalesceCache::new(dir.path());
        let key = command_key(&["ruff".into(), "check".into()], Path::new("/p"), &[]);

        let mut call_count = 0u32;

        // First call: exit_code 1 (failure). f() runs, result returns to caller,
        // but no entry is stored on disk.
        let r1: CachedResult = cache
            .with_lock(&key, || {
                call_count += 1;
                Ok(CachedResult { exit_code: 1, stdout: b"FAIL".to_vec(), stderr: vec![] })
            })
            .expect("first with_lock");
        assert_eq!(call_count, 1);
        assert_eq!(r1.exit_code, 1);
        assert!(
            !cache.entry_path(&key).exists(),
            "failed runs MUST NOT persist to disk (lane-7 BLOCKER)"
        );

        // Second call: f() MUST run again because the cache was not populated.
        let r2: CachedResult = cache
            .with_lock(&key, || {
                call_count += 1;
                Ok(CachedResult { exit_code: 0, stdout: b"OK".to_vec(), stderr: vec![] })
            })
            .expect("second with_lock");
        assert_eq!(call_count, 2, "failed-run must NOT short-circuit the next caller");
        assert_eq!(r2.exit_code, 0);

        // Third call: success IS cached, so f() does NOT run.
        let r3: CachedResult = cache
            .with_lock(&key, || {
                call_count += 1;
                Ok(CachedResult { exit_code: 99, stdout: b"won't run".to_vec(), stderr: vec![] })
            })
            .expect("third with_lock");
        assert_eq!(call_count, 2, "successful run IS cached, f() must not re-run");
        assert_eq!(r3.stdout, b"OK");
    }
}
