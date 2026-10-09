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

use fuser::{Errno, ReplyData, ReplyEmpty, ReplyEntry};

use super::InterceptFs;
use crate::inode_map::abs_under;

impl InterceptFs {
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
    pub fn link_rel(&self, existing: &Path, new: &Path) -> std::io::Result<()> {
        let (abs_existing, abs_new) =
            (abs_under(&self.backing, existing), abs_under(&self.backing, new));
        if let Some(parent) = abs_new.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::hard_link(&abs_existing, &abs_new)?;
        self.after_create_at(new, &abs_new)
    }

    /// Sync a live handle's descriptor when held, else open the backing path
    /// for `ino` and sync that. Durability primitive for `fsync`.
    pub fn fsync_handle(&self, ino: u64, fh: u64) -> std::io::Result<()> {
        if let Some(handle) = self.open_files.lock().expect("open files lock").get(&fh) {
            return handle.file.sync_all();
        }
        let path = self
            .inodes
            .lock()
            .expect("inode map lock")
            .abs_path(&self.backing, ino)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
        File::open(path)?.sync_all()
    }

    /// Advisory flush of a live handle: sync its descriptor when present and
    /// always report success. fuser calls `flush` on every `close()`, where an
    /// error cannot reach the caller, so a late sync failure must not surface
    /// on a write that already landed.
    pub fn flush_fh(&self, fh: u64) -> std::io::Result<()> {
        if let Some(handle) = self.open_files.lock().expect("open files lock").get(&fh) {
            let _ = handle.file.sync_data();
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
        let path = {
            let map = self.inodes.lock().expect("inode map");
            map.abs_path(&self.backing, ino)
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
        let Some(rel) = self.inodes.lock().expect("inode map").child_rel(parent, link_name) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let abs = abs_under(&self.backing, &rel);
        match std::os::unix::fs::symlink(target, &abs) {
            Ok(()) => {
                self.invalidate_neg_rel(&rel);
                self.install_created_entry_plain(rel, abs, reply);
            }
            Err(err) => reply.error(Self::io_errno(err)),
        }
    }

    /// `link` dispatch: create a hard link to an existing inode.
    pub(crate) fn op_link(&self, ino: u64, newparent: u64, newname: &OsStr, reply: ReplyEntry) {
        let existing = {
            let map = self.inodes.lock().expect("inode map");
            map.resolve(ino).map(Path::to_path_buf)
        };
        let Some(existing) = existing else {
            reply.error(Errno::ENOENT);
            return;
        };
        let Some(new_rel) = self.inodes.lock().expect("inode map").child_rel(newparent, newname)
        else {
            reply.error(Errno::ENOENT);
            return;
        };
        let (abs_existing, abs_new) =
            (abs_under(&self.backing, &existing), abs_under(&self.backing, &new_rel));
        match fs::hard_link(&abs_existing, &abs_new) {
            Ok(()) => {
                self.invalidate_neg_rel(&new_rel);
                self.install_created_entry_plain(new_rel, abs_new, reply);
            }
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
