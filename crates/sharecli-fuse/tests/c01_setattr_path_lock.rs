//! FR: FR-009
//!
//! Task 1.15 — FUSE setattr path lock.
//!
//! PLAN: "Add path lock around `setattr`; tests for concurrent `write`+`truncate`
//! on the same path."
//!
//! The macOS build uses fuser `macos-no-mount`, so the FUSE `setattr` op cannot
//! be driven through a live mount. The contract is asserted at the same
//! `InterceptFs` no-mount tier that 1.13/1.14 used: `setattr_rel` must take the
//! same per-path lock as `write_rel`, so a concurrent write and truncate on one
//! path cannot tear.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use sharecli_fuse::{InterceptFs, InterceptFsOptions};
use tempfile::TempDir;

fn seeded_backing(dir: &TempDir, name: &str, body: &[u8]) -> std::path::PathBuf {
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join(name), body).expect("seed");
    backing
}

/// Task 1.15 — truncate and chmod are applied through the setattr helper.
#[test]
fn setattr_truncate_and_mode_apply() {
    let dir = TempDir::new().expect("tempdir");
    let backing = seeded_backing(&dir, "f.txt", b"0123456789");
    let fs = InterceptFs::new(&backing);

    fs.setattr_rel(Path::new("f.txt"), Some(3), None).expect("truncate");
    assert_eq!(fs::metadata(backing.join("f.txt")).expect("meta").len(), 3);

    fs.setattr_rel(Path::new("f.txt"), None, Some(0o600)).expect("chmod");
    let mode = fs::metadata(backing.join("f.txt")).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "mode bits must be applied verbatim");
}

/// Task 1.15 — a concurrent write and truncate on one path leave a coherent
/// size (0 or the payload length), never a torn intermediate.
#[test]
fn concurrent_write_and_truncate_leave_a_coherent_size() {
    let dir = TempDir::new().expect("tempdir");
    let backing = seeded_backing(&dir, "race.txt", b"seed");
    let fs = Arc::new(InterceptFs::new(&backing));
    let payload = Arc::new(vec![b'x'; 64 * 1024]);

    for _ in 0..32 {
        let barrier = Arc::new(Barrier::new(2));
        let writer = {
            let fs = Arc::clone(&fs);
            let barrier = Arc::clone(&barrier);
            let payload = Arc::clone(&payload);
            thread::spawn(move || {
                barrier.wait();
                fs.write_rel(Path::new("race.txt"), 0, &payload).expect("write");
            })
        };
        let truncator = {
            let fs = Arc::clone(&fs);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                fs.setattr_rel(Path::new("race.txt"), Some(0), None).expect("truncate");
            })
        };
        writer.join().expect("writer");
        truncator.join().expect("truncator");

        let size = fs::metadata(backing.join("race.txt")).expect("meta").len();
        assert!(
            size == 0 || size == payload.len() as u64,
            "torn size after concurrent write+truncate: {size}"
        );
    }
}

/// Task 1.15 — `setattr_rel` contends on the same per-path lock as
/// `with_path_lock` (and therefore `write_rel`): the holder's critical section
/// completes before the truncate runs. Barrier-driven; no sleeps.
#[test]
fn setattr_serializes_against_the_path_lock() {
    let dir = TempDir::new().expect("tempdir");
    let backing = seeded_backing(&dir, "lock.txt", b"seed");
    let fs = Arc::new(InterceptFs::new(&backing));

    let barrier = Arc::new(Barrier::new(2));
    let order = Arc::new(Mutex::new(Vec::new()));

    let holder = {
        let fs = Arc::clone(&fs);
        let barrier = Arc::clone(&barrier);
        let order = Arc::clone(&order);
        thread::spawn(move || {
            fs.with_path_lock(Path::new("lock.txt"), || {
                order.lock().expect("order").push(1);
                barrier.wait();
                order.lock().expect("order").push(2);
            })
            .expect("lock holder");
        })
    };
    let truncator = {
        let fs = Arc::clone(&fs);
        let barrier = Arc::clone(&barrier);
        let order = Arc::clone(&order);
        thread::spawn(move || {
            barrier.wait();
            fs.setattr_rel(Path::new("lock.txt"), Some(0), None).expect("truncate");
            order.lock().expect("order").push(3);
        })
    };
    holder.join().expect("holder");
    truncator.join().expect("truncator");

    assert_eq!(*order.lock().expect("order"), vec![1, 2, 3], "setattr did not take the path lock");
}

/// Task 1.15 — `--no-serialize` mirrors `write_rel`: the mutation still applies
/// (the gate skips locking, it does not skip the operation).
#[test]
fn setattr_rel_applies_when_serialize_is_disabled() {
    let dir = TempDir::new().expect("tempdir");
    let backing = seeded_backing(&dir, "g.txt", b"abcdef");
    let fs = InterceptFs::with_options(
        &backing,
        InterceptFsOptions { serialize: false, ..Default::default() },
    );
    assert!(!fs.serialize_writes());
    fs.setattr_rel(Path::new("g.txt"), Some(2), None).expect("truncate");
    assert_eq!(fs::metadata(backing.join("g.txt")).expect("meta").len(), 2);
}

/// Task 1.15 — the path lock is released before the read cache is touched, so
/// setattr and a concurrent cached read never deadlock (bounded retries).
#[test]
fn setattr_does_not_deadlock_against_cached_reads() {
    let dir = TempDir::new().expect("tempdir");
    let backing = seeded_backing(&dir, "d.txt", b"payload");
    let fs = Arc::new(InterceptFs::new(&backing));
    let reads = Arc::new(AtomicUsize::new(0));

    let reader = {
        let fs = Arc::clone(&fs);
        let reads = Arc::clone(&reads);
        thread::spawn(move || {
            for _ in 0..64 {
                let _ = fs.read_coalesced_rel(Path::new("d.txt")).expect("read");
                reads.fetch_add(1, Ordering::Relaxed);
            }
        })
    };
    for _ in 0..64 {
        fs.setattr_rel(Path::new("d.txt"), Some(7), None).expect("setattr");
    }
    reader.join().expect("reader");
    assert_eq!(reads.load(Ordering::Relaxed), 64);
}
