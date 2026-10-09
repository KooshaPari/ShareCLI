//! FR: FR-009
//!
//! Task 1.14 — FUSE unlink keeps open fds readable (POSIX semantics).
//!
//! The macOS build uses fuser `macos-no-mount`, so a live FUSE mount is not
//! available on this host. The contract is expressed at the `InterceptFs`
//! no-mount handle tier (the same tier 1.13 used for the rename/resolve
//! contract): an open handle keeps reading the unlinked content after the
//! backing name is removed.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::Path;

use sharecli_fuse::InterceptFs;
use tempfile::TempDir;

/// Task 1.14 — an open handle survives backing unlink: the name is gone, but
/// the handle still reads the original bytes (POSIX "unlinked but open").
#[test]
fn unlink_keeps_open_handle_readable() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("a.txt"), b"unlinked-content").expect("seed");

    let fs = InterceptFs::new(&backing);
    let fh = fs.open_rel(Path::new("a.txt")).expect("open");
    assert_eq!(fs.read_at_handle(fh, 0, 64).expect("read before unlink"), b"unlinked-content");

    // FUSE unlink removes the backing directory entry.
    fs::remove_file(backing.join("a.txt")).expect("unlink");
    assert!(!backing.join("a.txt").exists(), "name must be gone after unlink");

    // POSIX: the already-open handle still reads the unlinked content.
    assert_eq!(
        fs.read_at_handle(fh, 0, 64).expect("read after unlink"),
        b"unlinked-content",
        "open handle lost the unlinked content"
    );
}

/// Task 1.14 — after unlink, a fresh open by name must fail (the name is really
/// gone; only the pre-existing handle survives).
#[test]
fn unlink_removes_the_name_for_fresh_opens() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("b.txt"), b"x").expect("seed");

    let fs = InterceptFs::new(&backing);
    fs::remove_file(backing.join("b.txt")).expect("unlink");
    assert!(fs.open_rel(Path::new("b.txt")).is_err(), "fresh open of a removed name must fail");
}

/// Task 1.14 — handle reads use absolute (pread) offsets, so a second read at a
/// different offset is not affected by the first.
#[test]
fn read_at_handle_uses_absolute_offset() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("c.txt"), b"0123456789").expect("seed");

    let fs = InterceptFs::new(&backing);
    let fh = fs.open_rel(Path::new("c.txt")).expect("open");
    assert_eq!(fs.read_at_handle(fh, 3, 4).expect("slice"), b"3456");
    assert_eq!(fs.read_at_handle(fh, 8, 4).expect("tail"), b"89");
    assert_eq!(fs.read_at_handle(fh, 20, 4).expect("past eof"), b"");
}

/// Task 1.14 x 1.13 — a rename that replaces an open name must not swap the
/// content the existing handle reads (POSIX: the old fd keeps the replaced
/// inode), while a fresh open sees the replacement.
#[test]
fn rename_over_open_handle_keeps_original_content() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("a.txt"), b"original").expect("seed a");
    fs::write(backing.join("b.txt"), b"replacement").expect("seed b");

    let fs = InterceptFs::new(&backing);
    let fh = fs.open_rel(Path::new("a.txt")).expect("open a.txt");
    // rename-over: b.txt replaces a.txt on the backing filesystem.
    fs::rename(backing.join("b.txt"), backing.join("a.txt")).expect("rename over");

    assert_eq!(
        fs.read_at_handle(fh, 0, 64).expect("read held handle"),
        b"original",
        "open handle must keep the replaced inode's content"
    );
    let fresh = fs.open_rel(Path::new("a.txt")).expect("open fresh");
    assert_eq!(fs.read_at_handle(fresh, 0, 64).expect("read fresh"), b"replacement");
}
