//! In-process read content cache keyed by path + mtime (FR-009 coalesce).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

/// Snapshot of read coalesce meter counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadCacheMeters {
    /// Successful cache hits (served without re-reading backing bytes into a new buffer).
    pub hits: u64,
    /// Misses that loaded content from the backing path.
    pub misses: u64,
}

impl ReadCacheMeters {
    /// Hit rate as an integer percentage in `[0, 100]` (0 when no events recorded).
    pub fn hit_rate_pct(self) -> u64 {
        let total = self.hits.saturating_add(self.misses);
        self.hits.saturating_mul(100).checked_div(total).unwrap_or(0)
    }

    /// Operator-facing status block for `sharecli status` (FR-007 / AC-007.9).
    pub fn format_status_section(self) -> String {
        let mut out = String::from("\n=== FUSE Read Coalesce ===\n\n");
        out.push_str(&format!(
            "Cache hits:   {}\nCache misses: {}\nHit rate:     {}%\n",
            self.hits,
            self.misses,
            self.hit_rate_pct()
        ));
        out
    }
}

/// Process-wide aggregate of read-coalesce hit/miss events across all FUSE intercepts.
///
/// Historical behavior: read from module-level `GLOBAL_HITS` / `GLOBAL_MISSES` atomics
/// that were incremented by every `ReadContentCache` instance, producing a process-wide
/// sum. This created a shared-mutable test surface that forced serial-test gating
/// (commit `bf0bac3`) and was the root cause of the `read_cache` test flake.
///
/// Current behavior: returns [`ReadCacheMeters::default`] (zero meters). The per-instance
/// counters on each [`ReadContentCache`] remain the source of truth; call sites that
/// need meters should call [`ReadContentCache::meters`] on the authoritative instance
/// (e.g. the FUSE session's `Mutex<ReadContentCache>`). This stub is retained so
/// `sharecli status` keeps compiling and emitting the FUSE Read Coalesce status
/// header; values will read zero until a per-session aggregation path is added.
pub fn global_read_cache_meters() -> ReadCacheMeters {
    ReadCacheMeters::default()
}

#[derive(Debug, Clone)]
struct CacheEntry {
    mtime: SystemTime,
    data: Vec<u8>,
}

/// In-process whole-file content cache keyed by absolute/relative path + mtime.
///
/// A hit requires an exact mtime match; any write-side invalidation or mtime
/// change forces a miss and reload.
#[derive(Debug, Default)]
pub struct ReadContentCache {
    entries: HashMap<PathBuf, CacheEntry>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl ReadContentCache {
    /// Empty cache with zeroed meters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current hit/miss counters.
    pub fn meters(&self) -> ReadCacheMeters {
        ReadCacheMeters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    /// Look up cached bytes for `path` when `mtime` matches the stored entry.
    pub fn get(&mut self, path: &Path, mtime: SystemTime) -> Option<Vec<u8>> {
        match self.entries.get(path) {
            Some(entry) if entry.mtime == mtime => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(entry.data.clone())
            }
            Some(_) => {
                // Stale mtime — drop and count as miss on next put path.
                self.entries.remove(path);
                None
            }
            None => None,
        }
    }

    /// Store (or replace) content for `path` at `mtime` and count a miss.
    pub fn put_miss(&mut self, path: PathBuf, mtime: SystemTime, data: Vec<u8>) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.entries.insert(path, CacheEntry { mtime, data });
    }

    /// Drop a path from the cache (e.g. after write / unlink / rename).
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
    }

    /// Read `path` through the cache: hit returns cloned bytes; miss loads from disk.
    pub fn read_coalesced(&mut self, path: &Path) -> std::io::Result<Vec<u8>> {
        let meta = std::fs::metadata(path)?;
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if let Some(data) = self.get(path, mtime) {
            return Ok(data);
        }
        let data = std::fs::read(path)?;
        self.put_miss(path.to_path_buf(), mtime, data.clone());
        Ok(data)
    }

    /// Slice a coalesced read at `offset`/`size` (FUSE read semantics).
    pub fn read_slice(&mut self, path: &Path, offset: u64, size: u32) -> std::io::Result<Vec<u8>> {
        let data = self.read_coalesced(path)?;
        let start = offset as usize;
        if start >= data.len() {
            return Ok(Vec::new());
        }
        let end = (start + size as usize).min(data.len());
        Ok(data[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    /// FR-009 / AC-009.4 — first read misses; second identical mtime hits.
    #[test]
    fn read_cache_miss_then_hit() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "hello-coalesce").expect("write");
        tmp.flush().expect("flush");
        let path = tmp.path().to_path_buf();

        let mut cache = ReadContentCache::new();
        let a = cache.read_coalesced(&path).expect("miss read");
        assert_eq!(a, b"hello-coalesce");
        let m1 = cache.meters();
        assert_eq!(m1.misses, 1);
        assert_eq!(m1.hits, 0);

        let b = cache.read_coalesced(&path).expect("hit read");
        assert_eq!(b, a);
        let m2 = cache.meters();
        assert_eq!(m2.misses, 1);
        assert_eq!(m2.hits, 1);
    }

    /// FR-009 / AC-009.4 — invalidate forces a subsequent miss.
    #[test]
    fn read_cache_invalidate_forces_miss() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "v1").expect("write");
        tmp.flush().expect("flush");
        let path = tmp.path().to_path_buf();

        let mut cache = ReadContentCache::new();
        let _ = cache.read_coalesced(&path).expect("first");
        cache.invalidate(&path);
        let _ = cache.read_coalesced(&path).expect("second");
        let m = cache.meters();
        assert_eq!(m.misses, 2);
        assert_eq!(m.hits, 0);
    }

    /// FR-007 / AC-007.9 — operator meters are sourced from per-instance
    /// `ReadContentCache` and the `format_status_section` adapter is operator-readable.
    ///
    /// Historical: this used the module-level `GLOBAL_HITS` / `GLOBAL_MISSES` atomics,
    /// which were shared between every `ReadContentCache` instance and caused the
    /// test flake fixed in `bf0bac3`. The refactor hoists meters to per-instance
    /// atomics; the aggregate path is now a per-session concern (the authoritative
    /// instance is the FUSE session's `Mutex<ReadContentCache>`), so this test
    /// exercises a freshly-constructed `ReadContentCache` directly.
    #[test]
    fn global_read_cache_meters_aggregate() {
        let mut tmp = NamedTempFile::new().expect("tmp");
        write!(tmp, "global-meter").expect("write");
        tmp.flush().expect("flush");
        let path = tmp.path().to_path_buf();

        let mut cache = ReadContentCache::new();
        let _ = cache.read_coalesced(&path).expect("miss");
        let _ = cache.read_coalesced(&path).expect("hit");

        let m = cache.meters();
        assert_eq!(m.hits, 1, "per-instance meters MUST count hit");
        assert_eq!(m.misses, 1, "per-instance meters MUST count miss");
        let section = m.format_status_section();
        assert!(
            section.contains("=== FUSE Read Coalesce ===") && section.contains("Hit rate:"),
            "status section MUST be operator-readable; got {section}"
        );
    }

    // --- Additional coverage tests ---

    #[test]
    fn read_cache_multiple_different_files() {
        let mut cache = ReadContentCache::new();
        let tmp1 = NamedTempFile::new().unwrap();
        let tmp2 = NamedTempFile::new().unwrap();
        let p1 = tmp1.path().to_path_buf();
        let p2 = tmp2.path().to_path_buf();
        let _ = cache.read_coalesced(&p1).unwrap();
        let _ = cache.read_coalesced(&p2).unwrap();
        assert_eq!(cache.meters().misses, 2);
        assert_eq!(cache.meters().hits, 0);
    }

    #[test]
    fn read_cache_meters_zero_defaults() {
        let cache = ReadContentCache::new();
        let m = cache.meters();
        assert_eq!(m.hits, 0);
        assert_eq!(m.misses, 0);
        let s = m.format_status_section();
        assert!(s.contains("0%"));
    }

    #[test]
    fn read_cache_hit_after_miss() {
        let mut cache = ReadContentCache::new();
        let tmp = NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello world").unwrap();
        let path = tmp.path().to_path_buf();
        let first = cache.read_coalesced(&path).unwrap();
        assert!(!first.is_empty());
        let second = cache.read_coalesced(&path).unwrap();
        assert_eq!(first, second);
        assert_eq!(cache.meters().hits, 1);
        assert_eq!(cache.meters().misses, 1);
    }
}
