//! FR: FR-009
//!
//! Task 1.18 (lane 6) — FUSE missing ops: symlink + link + readlink + flush +
//! fsync + release, so symlinked `node_modules` resolve and durability holds.
//!
//! PLAN (docs/audit/2026-09-20/PLAN.md:212-214): "Implement `readlink`, `symlink`,
//! `link`, `flush`, `fsync`, `release` so symlinked `node_modules` resolve and
//! durability holds."
//!
//! The macOS build uses fuser `macos-no-mount`, so a live FUSE mount is not
//! available on this host. The contract is expressed at the `InterceptFs`
//! no-mount helper tier (the same tier 1.13/1.14/1.15 used), where the backing
//! filesystem effects are exact and deterministic.
//!
//! `release` (drop the live handle on close) landed with task 1.14; 1.18 asserts
//! its behaviour rather than re-implementing it. FR/AC ids: FR-009 is the owning
//! FR; no AC id covers these ops yet, so none is cited (see artifact).

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use sharecli_fuse::InterceptFs;
use tempfile::TempDir;

fn backing_dir(dir: &TempDir) -> std::path::PathBuf {
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    backing
}

/// FR-009 / task 1.18 — `symlink` creates a symlink whose target text is read
/// back verbatim by `readlink` (no resolution, no rewrite). A `node_modules`
/// style relative target is not followed, so a dangling link still reads back.
#[test]
fn symlink_readlink_roundtrip_preserves_target_text() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    let fs = InterceptFs::new(&backing);

    fs.symlink_rel(Path::new("../pkg/bin.js"), Path::new("node_modules/.bin/pkg"))
        .expect("symlink");

    let link = backing.join("node_modules/.bin/pkg");
    let meta = fs::symlink_metadata(&link).expect("lstat link");
    assert!(meta.is_symlink(), "link must be a symlink, not its target");
    assert_eq!(
        fs.readlink_rel(Path::new("node_modules/.bin/pkg")).expect("readlink"),
        Path::new("../pkg/bin.js"),
        "readlink must return the stored target text verbatim"
    );
    // The target does not exist on the backing: readlink must not follow it.
    assert!(!backing.join("node_modules/.bin/../pkg/bin.js").exists());
}

/// FR-009 / task 1.18 — creating a symlink under a missing directory creates
/// the parent, mirroring `create_rel`'s mkdir semantics.
#[test]
fn symlink_rel_creates_missing_parent_directories() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    let fs = InterceptFs::new(&backing);

    fs.symlink_rel(Path::new("a"), Path::new("x/y/z/link")).expect("symlink nested");
    assert!(fs::symlink_metadata(backing.join("x/y/z/link")).expect("lstat").is_symlink());
}

/// FR-009 / task 1.18 — `symlink` invalidates a stale negative dentry for the
/// new link, so a fresh existence probe observes it.
#[test]
fn symlink_rel_invalidates_negative_dentry() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    let fs = InterceptFs::new(&backing);
    fs::write(backing.join("real.txt"), b"target").expect("seed target");

    assert!(!fs.exists_rel(Path::new("link")).expect("probe missing"));
    fs.symlink_rel(Path::new("real.txt"), Path::new("link")).expect("symlink");
    assert!(
        fs.exists_rel(Path::new("link")).expect("probe after symlink"),
        "a stale negative dentry must not hide the new link"
    );
}

/// FR-009 / task 1.18 — `link` creates a second name for the same inode: both
/// names share content and observe each other's writes.
#[test]
fn link_creates_hardlink_sharing_the_same_inode() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    fs::write(backing.join("a.txt"), b"shared").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.link_rel(Path::new("a.txt"), Path::new("b.txt")).expect("hard link");

    let (a, b) = (
        fs::symlink_metadata(backing.join("a.txt")).expect("a"),
        fs::symlink_metadata(backing.join("b.txt")).expect("b"),
    );
    assert_eq!(a.ino(), b.ino(), "hard links must share one inode");
    assert_eq!(b.nlink(), 2, "both names must count toward the link count");
    fs::write(backing.join("b.txt"), b"rewritten").expect("write via link");
    assert_eq!(fs::read(backing.join("a.txt")).expect("read a"), b"rewritten");
}

/// FR-009 / task 1.18 — a hard link to a missing source fails loudly and does
/// not leave a stray name behind.
#[test]
fn link_rel_rejects_missing_source_without_leaving_a_name() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    let fs = InterceptFs::new(&backing);

    let err = fs.link_rel(Path::new("missing"), Path::new("x")).expect_err("must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(!backing.join("x").exists(), "no dangling name on failure");
}

/// FR-009 / task 1.18 — `fsync` flushes a live handle's bytes to stable
/// storage and reports unknown handles rather than pretending success.
#[test]
fn fsync_handle_syncs_live_handle_and_errors_on_unknown() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    fs::write(backing.join("f.txt"), b"durable").expect("seed");
    let fs = InterceptFs::new(&backing);

    let fh = fs.open_rel(Path::new("f.txt")).expect("open");
    fs.fsync_handle(fh, fh).expect("fsync live handle");
    assert_eq!(fs::read(backing.join("f.txt")).expect("read"), b"durable");

    let err = fs.fsync_handle(99_991, 99_991).expect_err("unknown handle must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

/// FR-009 / task 1.18 — `flush` syncs a live handle and is always `Ok`, even
/// for an unknown/duplicated handle (fuser flush semantics: close cannot report
/// an error, and ENOSYS/EIO here would surface on a landed write).
#[test]
fn flush_fh_syncs_live_handle_and_is_ok_when_absent() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    fs::write(backing.join("h.txt"), b"held").expect("seed");
    let fs = InterceptFs::new(&backing);

    let fh = fs.open_rel(Path::new("h.txt")).expect("open");
    fs.flush_fh(fh).expect("flush live handle");
    fs.flush_fh(fh.wrapping_add(4096)).expect("flush unknown handle must be Ok");
}

/// FR-009 / task 1.14 x 1.18 — `release` drops the live handle: the
/// unlinked-but-open read guarantee holds until release, then the handle is
/// gone and a later read cannot resurrect it.
#[test]
fn release_fh_drops_the_live_handle() {
    let dir = TempDir::new().expect("tempdir");
    let backing = backing_dir(&dir);
    fs::write(backing.join("a.txt"), b"unlinked-content").expect("seed");
    let fs = InterceptFs::new(&backing);

    let fh = fs.open_rel(Path::new("a.txt")).expect("open");
    fs::remove_file(backing.join("a.txt")).expect("unlink");
    assert_eq!(fs.read_at_handle(fh, 0, 64).expect("held fd survives unlink"), b"unlinked-content");

    assert!(fs.release_fh(fh), "release must drop a live handle");
    assert!(!fs.release_fh(fh), "release must be idempotent");
    assert!(
        fs.read_at_handle(fh, 0, 64).is_err(),
        "after release the dropped handle must not serve reads"
    );
}
