//! Task 1.18 (lane 6) — inherent helpers for the FUSE ops the passthrough
//! filesystem was missing: `readlink`, `symlink`, `link`, `flush`, and `fsync`,
//! plus the task-1.14 `release` handle drop.
//!
//! PLAN (`docs/audit/2026-09-20/PLAN.md:212-214`): "Implement `readlink`,
//! `symlink`, `link`, `flush`, `fsync`, `release` so symlinked `node_modules`
//! resolve and durability holds."  FR-009 is the owning requirement; no AC id
//! covers these ops yet, so none is invented here.
//!
//! Rust requires a trait impl to be a single block, so the `Filesystem`
//! dispatch for these ops lives in `lib.rs`; this submodule (a descendant of
//! `platform`, so it can use the filesystem's private fields) holds only the
//! inherent helpers, keeping `lib.rs` growth within budget.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use fuser::{Errno, Generation, ReplyData, ReplyEmpty, ReplyEntry};

use super::{InterceptFs, TTL};
use crate::inode_map::{abs_under, InodeMap};

/// Recoverable error for a poisoned lock (maps to `Errno::EIO`): a handler must
/// reply with an error rather than panic, which would unwind the FUSE session.
fn eio() -> std::io::Error {
    std::io::Error::other("sharecli-fuse: lock poisoned")
}

/// A stale/unknown file handle (maps to `Errno::EBADF`). EBADF is 9 across the
/// Unix targets this module compiles for.
fn ebadf() -> std::io::Error {
    std::io::Error::from_raw_os_error(9)
}

impl InterceptFs {
    /// Acquire the inode map, converting a poisoned mutex into a recoverable
    /// [`eio`] error instead of panicking inside a request handler.
    fn inode_map(&self) -> std::io::Result<MutexGuard<'_, InodeMap>> {
        self.inodes.lock().map_err(|_| eio())
    }

    /// Create a symlink at relative `link` storing `target` verbatim (a
    /// relative target is resolved by the kernel relative to `link`'s
    /// directory, so a `node_modules/.bin` style link is not rewritten).
    ///
    /// Missing parent directories are created, mirroring
    /// [`InterceptFs::create_rel`].
    pub fn symlink_rel(&self, target: &Path, link: &Path) -> std::io::Result<()> {
        let abs = abs_under(&self.backing, link);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        std::os::unix::fs::symlink(target, &abs)?;
        self.after_create_at(link, &abs)
    }

    /// Read the stored target of the symlink at relative `link` without
    /// following it.
    pub fn readlink_rel(&self, link: &Path) -> std::io::Result<PathBuf> {
        fs::read_link(abs_under(&self.backing, link))
    }

    /// Create a hard link: relative `new` becomes a second name for the inode
    /// behind relative `existing`.
    ///
    /// POSIX: both names must resolve to the SAME inode, so `new` is registered
    /// as an alias of the source inode rather than receiving a fresh allocation.
    pub fn link_rel(&self, existing: &Path, new: &Path) -> std::io::Result<()> {
        let (abs_existing, abs_new) =
            (abs_under(&self.backing, existing), abs_under(&self.backing, new));
        if let Some(parent) = abs_new.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::hard_link(&abs_existing, &abs_new)?;
        {
            let mut map = self.inode_map()?;
            let src_ino = map.alloc_or_get(existing.to_path_buf());
            map.alias_rel(new.to_path_buf(), src_ino);
        }
        self.after_create_at(new, &abs_new)
    }

    /// Sync a live handle's descriptor when held, else open the backing path
    /// for `ino` and sync that. Durability primitive for `fsync`.
    ///
    /// A nonzero `fh` must name a live handle: an unknown one is [`ebadf`],
    /// never silently resolved through `ino` (which would sync a *different*
    /// file and report success). The descriptor is cloned under the lock and
    /// synced after the lock is released, so the syscall is not serialized
    /// behind the global `open_files` mutex and the fd is pinned across the
    /// sync (no path re-resolve / TOCTOU).
    pub fn fsync_handle(&self, ino: u64, fh: u64) -> std::io::Result<()> {
        if fh != 0 {
            let held = self
                .open_files
                .lock()
                .map_err(|_| eio())?
                .get(&fh)
                .map(|h| h.file.try_clone())
                .ok_or_else(ebadf)??;
            return held.sync_all();
        }
        let path = self
            .inode_map()?
            .abs_path(&self.backing, ino)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
        File::open(path)?.sync_all()
    }

    /// Advisory flush of a live handle: sync its descriptor when present and
    /// always report success. fuser calls `flush` on every `close()`, where an
    /// error cannot reach the caller, so a late sync failure must not surface
    /// on a write that already landed. The descriptor is cloned under the lock
    /// and synced after it is released.
    pub fn flush_fh(&self, fh: u64) -> std::io::Result<()> {
        let cloned = self
            .open_files
            .lock()
            .ok()
            .and_then(|guard| guard.get(&fh).and_then(|h| h.file.try_clone().ok()));
        if let Some(file) = cloned {
            let _ = file.sync_data();
        }
        Ok(())
    }

    /// Drop the live handle for `fh` (the task 1.14 release mechanism). Returns
    /// whether a handle was actually removed; a second call is a no-op.
    pub fn release_fh(&self, fh: u64) -> bool {
        self.open_files.lock().map(|mut open| open.remove(&fh).is_some()).unwrap_or(false)
    }

    /// `readlink` dispatch: return the stored symlink target (no follow).
    pub(crate) fn op_readlink(&self, ino: u64, reply: ReplyData) {
        let path = match self.inode_map() {
            Ok(map) => map.abs_path(&self.backing, ino),
            Err(err) => {
                reply.error(Self::io_errno(err));
                return;
            }
        };
        match path {
            Some(path) => match fs::read_link(&path) {
                Ok(target) => reply.data(target.as_os_str().as_bytes()),
                Err(err) => reply.error(Self::io_errno(err)),
            },
            None => reply.error(Errno::ENOENT),
        }
    }

    /// `symlink` dispatch: create a symlink; a relative `target` is stored
    /// verbatim.
    pub(crate) fn op_symlink(
        &self,
        parent: u64,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        let rel = match self.inode_map() {
            Ok(map) => map.child_rel(parent, link_name),
            Err(err) => {
                reply.error(Self::io_errno(err));
                return;
            }
        };
        let Some(rel) = rel else {
            reply.error(Errno::ENOENT);
            return;
        };
        let abs = abs_under(&self.backing, &rel);
        // Delegate to `symlink_rel` so the handler matches the helper: missing
        // parent directories are created and the negative-dentry / read caches
        // are invalidated by `after_create_at`.
        match self.symlink_rel(target, &rel) {
            Ok(()) => self.install_created_entry_plain(rel, abs, reply),
            Err(err) => reply.error(Self::io_errno(err)),
        }
    }

    /// `link` dispatch: create a hard link to an existing inode.
    pub(crate) fn op_link(&self, ino: u64, newparent: u64, newname: &OsStr, reply: ReplyEntry) {
        let existing = match self.inode_map() {
            Ok(map) => map.resolve(ino).map(Path::to_path_buf),
            Err(err) => {
                reply.error(Self::io_errno(err));
                return;
            }
        };
        let Some(existing) = existing else {
            reply.error(Errno::ENOENT);
            return;
        };
        let new_rel = match self.inode_map() {
            Ok(map) => map.child_rel(newparent, newname),
            Err(err) => {
                reply.error(Self::io_errno(err));
                return;
            }
        };
        let Some(new_rel) = new_rel else {
            reply.error(Errno::ENOENT);
            return;
        };
        let (abs_existing, abs_new) =
            (abs_under(&self.backing, &existing), abs_under(&self.backing, &new_rel));
        match fs::hard_link(&abs_existing, &abs_new) {
            Ok(()) => {
                self.invalidate_neg_rel(&new_rel);
                self.install_linked_entry(new_rel, ino, abs_new, reply);
            }
            Err(err) => reply.error(Self::io_errno(err)),
        }
    }

    /// Reply with the entry for a name just created as a hard link, registering
    /// it as an alias of the SOURCE inode (POSIX) rather than allocating a new
    /// inode. `symlink_metadata` keeps the link's own kind in the reply.
    fn install_linked_entry(&self, rel: PathBuf, ino: u64, path: PathBuf, reply: ReplyEntry) {
        match self.inode_map() {
            Ok(mut map) => map.alias_rel(rel, ino),
            Err(err) => {
                reply.error(Self::io_errno(err));
                return;
            }
        }
        match fs::symlink_metadata(&path) {
            Ok(meta) => reply.entry(&TTL, &Self::metadata_to_attr(ino, &meta), Generation(0)),
            Err(err) => reply.error(Self::io_errno(err)),
        }
    }

    /// `flush` dispatch: advisory flush on close, always succeeds.
    pub(crate) fn op_flush(&self, fh: u64, reply: ReplyEmpty) {
        let _ = self.flush_fh(fh);
        reply.ok();
    }

    /// `fsync` dispatch: sync file contents for durability.
    pub(crate) fn op_fsync(&self, ino: u64, fh: u64, reply: ReplyEmpty) {
        match self.fsync_handle(ino, fh) {
            Ok(()) => reply.ok(),
            Err(err) => reply.error(Self::io_errno(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CRITICAL fix — `link` must register the new name against the SOURCE
    /// path's existing inode (POSIX hard-link alias). Allocating a fresh inode
    /// for `new` breaks the invariant that both names denote one inode.
    #[test]
    fn link_rel_aliases_the_source_inode() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let backing = dir.path().join("back");
        fs::create_dir_all(&backing).expect("backing");
        fs::write(backing.join("a.txt"), b"shared").expect("seed");
        let fs = InterceptFs::new(&backing);

        let src_ino = fs.inodes.lock().expect("inode map").alloc_or_get(PathBuf::from("a.txt"));
        fs.link_rel(Path::new("a.txt"), Path::new("b.txt")).expect("hard link");

        let new_ino = fs.inodes.lock().expect("inode map").alloc_or_get(PathBuf::from("b.txt"));
        assert_eq!(new_ino, src_ino, "hard link allocated a fresh inode instead of aliasing");
    }

    /// lib.rs keying — `read_at_handle` is keyed by the per-open handle only. An
    /// `fh` that was never allocated must NOT be reinterpreted as an inode
    /// number (which would read a different file than the caller asked for).
    #[test]
    fn read_at_handle_rejects_unallocated_handle() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let backing = dir.path().join("back");
        fs::create_dir_all(&backing).expect("backing");
        fs::write(backing.join("a.txt"), b"AAA").expect("seed");
        let fs = InterceptFs::new(&backing);

        let ino = fs.inodes.lock().expect("inode map").alloc_or_get(PathBuf::from("a.txt"));
        assert!(ino < super::super::FIRST_OPEN_FH, "premise: inodes are below the handle base");
        assert!(
            fs.read_at_handle(ino, 0, 3).is_err(),
            "an unallocated handle must not resolve through the inode map"
        );
    }

    /// ops_missing — `fsync` for a live inode but an unknown (released) handle
    /// must fail with EBADF, not sync a different file and report success.
    #[test]
    fn fsync_handle_rejects_unknown_handle_with_ebadf() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let backing = dir.path().join("back");
        fs::create_dir_all(&backing).expect("backing");
        fs::write(backing.join("f.txt"), b"durable").expect("seed");
        let fs = InterceptFs::new(&backing);

        let fh = fs.open_rel(Path::new("f.txt")).expect("open");
        let ino = fs.inodes.lock().expect("inode map").alloc_or_get(PathBuf::from("f.txt"));
        assert!(fs.release_fh(fh), "handle must be live before release");

        let err = fs.fsync_handle(ino, fh).expect_err("released handle must fail");
        assert_eq!(err.raw_os_error(), Some(9), "unknown fh must surface EBADF");
    }
}
