//! Per-path write serialization + staging CoW commit/discard (FR-009).
//!
//! Concurrent writes to the same path take a per-path mutex. Staging CoW
//! writes land under `staging_root` (hashed by absolute backing path); callers
//! promote with [`WriteSerialize::commit_pending`] or drop with
//! [`WriteSerialize::discard_pending`].

use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use crate::write_serialize_meters::{record_commit, record_discard, record_stage};

/// Error from write-serialize / CoW commit-discard operations.
#[derive(Debug, thiserror::Error)]
pub enum WriteSerializeError {
    /// Underlying filesystem IO failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// No pending staging file for the given backing path.
    #[error("write-serialize: no pending CoW staging for {0}")]
    NoPending(PathBuf),
    /// Internal lock poisoning.
    #[error("write-serialize lock poisoned")]
    Poisoned,
}

/// Serialize concurrent writes and hold CoW staging copies per backing path.
#[derive(Debug)]
pub struct WriteSerialize {
    staging_root: PathBuf,
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    pending: Mutex<HashMap<PathBuf, PathBuf>>,
}

impl Default for WriteSerialize {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteSerialize {
    /// Create with a unique staging directory under the process temp dir.
    pub fn new() -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let staging_root =
            std::env::temp_dir().join(format!("sharecli-cow-{}-{}", std::process::id(), nanos));
        Self::with_staging_root(staging_root)
    }

    /// Create with an explicit staging root (created if missing).
    pub fn with_staging_root(staging_root: impl Into<PathBuf>) -> Self {
        let staging_root = staging_root.into();
        let _ = fs::create_dir_all(&staging_root);
        Self {
            staging_root,
            locks: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Staging directory root.
    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }

    fn lock_arc(&self, path: &Path) -> Result<Arc<Mutex<()>>, WriteSerializeError> {
        let mut map = self.locks.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        let entry =
            map.entry(path.to_path_buf()).or_insert_with(|| Arc::new(Mutex::new(()))).clone();
        Ok(entry)
    }

    /// Run `f` while holding the exclusive write lock for `path`.
    pub fn with_locked_path<R, F: FnOnce() -> R>(
        &self,
        path: &Path,
        f: F,
    ) -> Result<R, WriteSerializeError> {
        let arc = self.lock_arc(path)?;
        let _guard = arc.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        Ok(f())
    }

    fn staging_path_for(&self) -> PathBuf {
        self.staging_root.join(Self::unique_staging_name())
    }

    /// Collision-safe name for one staging file: pid + wall-clock nanos + a
    /// process-local counter, so no two staging operations share a file name
    /// (replaces the previous hash-of-backing-path name).
    fn unique_staging_name() -> String {
        let seq = STAGE_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        format!(".sharecli-staging-{}-{}-{seq:016x}", std::process::id(), nanos)
    }

    /// Create a new staging file under the staging root, retrying on the
    /// (astronomically unlikely) name collision.
    fn create_unique_staging(&self) -> Result<(File, PathBuf), WriteSerializeError> {
        const ATTEMPTS: usize = 16;
        fs::create_dir_all(&self.staging_root)?;
        for _ in 0..ATTEMPTS {
            let candidate = self.staging_path_for();
            match fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(file) => return Ok((file, candidate)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(WriteSerializeError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "write-serialize: could not allocate a unique staging file",
        )))
    }

    /// Write `contents` into a fresh, uniquely named staging file for `backing`.
    ///
    /// Marks the path pending and removes any superseded staging file. Does not
    /// modify the backing file.
    pub fn stage_bytes(&self, backing: &Path, contents: &[u8]) -> Result<(), WriteSerializeError> {
        let backing = backing.to_path_buf();
        let arc = self.lock_arc(&backing)?;
        let _guard = arc.lock().map_err(|_| WriteSerializeError::Poisoned)?;

        let (mut file, staging) = self.create_unique_staging()?;
        file.write_all(contents)?;
        file.sync_all()?;

        let superseded = {
            let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending.insert(backing, staging)
        };
        if let Some(prev) = superseded {
            let _ = fs::remove_file(prev);
        }
        record_stage();
        Ok(())
    }

    /// Atomically promote the staging copy to `backing` (rename/replace).
    ///
    /// Removes the pending entry and releases the path lock entry after success.
    pub fn commit_pending(&self, backing: &Path) -> Result<(), WriteSerializeError> {
        let backing_buf = backing.to_path_buf();
        let arc = self.lock_arc(&backing_buf)?;
        let _guard = arc.lock().map_err(|_| WriteSerializeError::Poisoned)?;

        let staging = {
            let pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending
                .get(&backing_buf)
                .cloned()
                .ok_or_else(|| WriteSerializeError::NoPending(backing_buf.clone()))?
        };

        if !staging.exists() {
            let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending.remove(&backing_buf);
            return Err(WriteSerializeError::NoPending(backing_buf));
        }

        if let Some(parent) = backing_buf.parent() {
            fs::create_dir_all(parent)?;
        }

        // Same-filesystem atomic replace; on EXDEV stage next to the
        // destination and rename within that directory (atomic replace).
        match fs::rename(&staging, &backing_buf) {
            Ok(()) => {}
            Err(err) if err.raw_os_error() == Some(libc_exdev()) => {
                self.promote_via_destination_dir(&backing_buf, &staging)?;
            }
            Err(err) => return Err(err.into()),
        }

        let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        pending.remove(&backing_buf);
        record_commit();
        Ok(())
    }

    /// Delete the staging file for `backing` without touching the backing path.
    ///
    /// Returns [`WriteSerializeError::NoPending`] when nothing is staged.
    pub fn discard_pending(&self, backing: &Path) -> Result<(), WriteSerializeError> {
        let backing_buf = backing.to_path_buf();
        let arc = self.lock_arc(&backing_buf)?;
        let _guard = arc.lock().map_err(|_| WriteSerializeError::Poisoned)?;

        let staging = {
            let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending
                .remove(&backing_buf)
                .ok_or_else(|| WriteSerializeError::NoPending(backing_buf.clone()))?
        };

        if staging.exists() {
            fs::remove_file(&staging)?;
        }
        record_discard();
        Ok(())
    }

    /// Whether `backing` currently has a pending staging file.
    pub fn has_pending(&self, backing: &Path) -> Result<bool, WriteSerializeError> {
        let pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        Ok(pending.contains_key(backing))
    }

    /// Backing paths that currently have staged CoW bytes pending commit/discard.
    pub fn pending_backing_paths(&self) -> Result<Vec<PathBuf>, WriteSerializeError> {
        let pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        Ok(pending.keys().cloned().collect())
    }

    /// Staging file currently held for `backing`, if any.
    pub fn pending_staging_path(
        &self,
        backing: &Path,
    ) -> Result<Option<PathBuf>, WriteSerializeError> {
        let pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        Ok(pending.get(backing).cloned())
    }

    /// Staging path used by the EXDEV fallback: created next to the destination
    /// so the final promote is a same-filesystem, atomic rename (PLAN.md:206).
    pub fn exdev_staging_path(&self, backing: &Path) -> PathBuf {
        let dir = match backing.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        dir.join(Self::unique_staging_name())
    }

    /// Cross-device promote: copy `staging` next to `backing`, then atomically
    /// rename it over the destination (never leaves a partially written file).
    /// Caller must already hold the per-path lock for `backing`.
    fn promote_via_destination_dir(
        &self,
        backing: &Path,
        staging: &Path,
    ) -> Result<(), WriteSerializeError> {
        if let Some(parent) = backing.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = self.exdev_staging_path(backing);
        fs::copy(staging, &temp)?;
        if let Err(err) = fs::rename(&temp, backing) {
            let _ = fs::remove_file(&temp);
            return Err(err.into());
        }
        let _ = fs::remove_file(staging);
        Ok(())
    }

    /// Commit through the EXDEV path explicitly (cross-device staging).
    ///
    /// Equivalent to [`Self::commit_pending`] when the same-filesystem rename
    /// cannot be used; exposed so the fallback is testable without a second
    /// mounted filesystem.
    pub fn commit_pending_exdev(&self, backing: &Path) -> Result<(), WriteSerializeError> {
        let backing_buf = backing.to_path_buf();
        let arc = self.lock_arc(&backing_buf)?;
        let _guard = arc.lock().map_err(|_| WriteSerializeError::Poisoned)?;

        let staging = {
            let pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending
                .get(&backing_buf)
                .cloned()
                .ok_or_else(|| WriteSerializeError::NoPending(backing_buf.clone()))?
        };
        if !staging.exists() {
            let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
            pending.remove(&backing_buf);
            return Err(WriteSerializeError::NoPending(backing_buf));
        }

        self.promote_via_destination_dir(&backing_buf, &staging)?;

        let mut pending = self.pending.lock().map_err(|_| WriteSerializeError::Poisoned)?;
        pending.remove(&backing_buf);
        record_commit();
        Ok(())
    }
}

/// EXDEV (cross-device link) — portable constant (POSIX / Linux / macOS).
fn libc_exdev() -> i32 {
    18
}

/// Process-local counter making staging file names unique across operations.
static STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    /// FR-009 / AC-009.5 — stage → commit promotes; stage → discard leaves backing.
    #[test]
    fn write_serialize_stage_commit_and_discard() {
        let dir = TempDir::new().expect("tempdir");
        let staging = dir.path().join("staging");
        let ws = WriteSerialize::with_staging_root(&staging);

        let backing = dir.path().join("file.txt");
        fs::write(&backing, b"original").expect("seed");

        ws.stage_bytes(&backing, b"committed").expect("stage");
        assert!(ws.has_pending(&backing).expect("pending"));
        ws.commit_pending(&backing).expect("commit");
        assert_eq!(fs::read(&backing).expect("read"), b"committed");
        assert!(!ws.has_pending(&backing).expect("cleared"));

        fs::write(&backing, b"keep-me").expect("reset");
        ws.stage_bytes(&backing, b"should-discard").expect("stage2");
        ws.discard_pending(&backing).expect("discard");
        assert_eq!(fs::read(&backing).expect("unchanged"), b"keep-me");
        assert!(!ws.has_pending(&backing).expect("cleared2"));

        assert!(matches!(ws.discard_pending(&backing), Err(WriteSerializeError::NoPending(_))));
        assert!(matches!(ws.commit_pending(&backing), Err(WriteSerializeError::NoPending(_))));
    }

    /// FR-009 / AC-009.5 — same-path writers serialize (no overlapping critical section).
    #[test]
    fn write_serialize_serializes_same_path() {
        let ws = Arc::new(WriteSerialize::new());
        let barrier = Arc::new(Barrier::new(2));
        let path = PathBuf::from("/virtual/same");
        let order = Arc::new(Mutex::new(Vec::new()));

        let ws1 = Arc::clone(&ws);
        let b1 = Arc::clone(&barrier);
        let o1 = Arc::clone(&order);
        let p1 = path.clone();
        let t1 = thread::spawn(move || {
            ws1.with_locked_path(&p1, || {
                o1.lock().expect("order").push(1);
                b1.wait();
                thread::sleep(Duration::from_millis(40));
                o1.lock().expect("order").push(2);
            })
            .expect("lock");
        });

        let ws2 = Arc::clone(&ws);
        let b2 = Arc::clone(&barrier);
        let o2 = Arc::clone(&order);
        let p2 = path;
        let t2 = thread::spawn(move || {
            b2.wait();
            ws2.with_locked_path(&p2, || {
                o2.lock().expect("order").push(3);
            })
            .expect("lock");
        });

        t1.join().expect("t1");
        t2.join().expect("t2");
        let seq = order.lock().expect("order").clone();
        assert_eq!(seq, vec![1, 2, 3]);
    }
}
