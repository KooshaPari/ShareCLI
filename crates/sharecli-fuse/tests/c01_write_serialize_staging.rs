//! FR: FR-009
//!
//! Task 1.16 — FUSE staging collision-safe.
//!
//! PLAN (docs/audit/2026-09-20/PLAN.md:204-206): "`tempdir()` per staging, unique
//! names; EXDEV fallback stages into destination directory."
//!
//! Deterministic (no mount, no sleeps): the WriteSerialize staging tier is pure
//! filesystem, so every assertion here is exact.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::Path;

use sharecli_fuse::WriteSerialize;
use tempfile::TempDir;

fn staging_root(dir: &TempDir) -> std::path::PathBuf {
    let root = dir.path().join("staging");
    fs::create_dir_all(&root).expect("staging root");
    root
}

fn dir_entries(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// FR-009 — every staging operation gets a unique staging file: re-staging the
/// same backing never reuses the previous name, and distinct backings never
/// share one (regression for the hash-collision class, FINDINGS L121).
#[test]
fn staging_paths_are_unique_per_stage() {
    let dir = TempDir::new().expect("tempdir");
    let root = staging_root(&dir);
    let ws = WriteSerialize::with_staging_root(&root);

    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");

    ws.stage_bytes(&a, b"one").expect("stage a1");
    let a1 = ws.pending_staging_path(&a).expect("pending a1").expect("a1 path");
    ws.stage_bytes(&a, b"two").expect("stage a2");
    let a2 = ws.pending_staging_path(&a).expect("pending a2").expect("a2 path");
    ws.stage_bytes(&b, b"three").expect("stage b1");
    let b1 = ws.pending_staging_path(&b).expect("pending b1").expect("b1 path");

    assert_ne!(a1, a2, "re-staging the same backing must allocate a fresh name");
    assert_ne!(a1, b1, "distinct backings must not share a staging file");
    assert_ne!(a2, b1, "distinct backings must not share a staging file");
    for p in [&a1, &a2, &b1] {
        assert_eq!(p.parent(), Some(root.as_path()), "staging file must live under staging_root");
    }
    assert_eq!(ws.pending_backing_paths().expect("pending").len(), 2);
}

/// FR-009 — re-staging supersedes and removes the previous staging file, so
/// unique names do not leak orphans in the staging root.
#[test]
fn stage_supersede_removes_previous_staging_file() {
    let dir = TempDir::new().expect("tempdir");
    let root = staging_root(&dir);
    let ws = WriteSerialize::with_staging_root(&root);

    let a = dir.path().join("a.txt");
    ws.stage_bytes(&a, b"one").expect("stage 1");
    let first = ws.pending_staging_path(&a).expect("pending").expect("path");
    ws.stage_bytes(&a, b"two").expect("stage 2");
    let second = ws.pending_staging_path(&a).expect("pending").expect("path");

    assert_ne!(first, second);
    assert!(!first.exists(), "superseded staging file must be removed");
    assert!(second.exists(), "current staging file must exist");
    assert_eq!(
        dir_entries(&root),
        vec![second.file_name().unwrap().to_string_lossy().into_owned()]
    );
}

/// FR-009 — the EXDEV fallback lands the full staged bytes (never partial or
/// zero-length), removes the staging file, and clears the pending entry.
#[test]
fn commit_exdev_fallback_lands_full_bytes_and_clears_pending() {
    let dir = TempDir::new().expect("tempdir");
    let root = staging_root(&dir);
    let ws = WriteSerialize::with_staging_root(&root);

    let backing = dir.path().join("dest").join("file.txt");
    fs::create_dir_all(backing.parent().expect("dest dir")).expect("dest dir");
    fs::write(&backing, b"original-contents").expect("seed");

    ws.stage_bytes(&backing, b"replacement-bytes").expect("stage");
    let staged = ws.pending_staging_path(&backing).expect("pending").expect("path");

    // Exercise the cross-device fallback directly (cannot mount a second
    // filesystem in a unit test).
    ws.commit_pending_exdev(&backing).expect("exdev commit");

    assert_eq!(fs::read(&backing).expect("read"), b"replacement-bytes", "full bytes must land");
    assert!(!ws.has_pending(&backing).expect("pending cleared"));
    assert!(!staged.exists(), "staging file removed after commit");
    assert_eq!(dir_entries(&root), Vec::<String>::new(), "no staging leftovers");
    let dest_leftovers: Vec<String> = dir_entries(backing.parent().expect("dest"));
    assert_eq!(
        dest_leftovers,
        vec!["file.txt".to_string()],
        "no destination-directory staging leftovers"
    );
}

/// FR-009 — the EXDEV staging path is created next to the destination so the
/// final promote is a same-filesystem, atomic rename (PLAN.md:206).
#[test]
fn exdev_staging_path_lands_in_destination_dir() {
    let dir = TempDir::new().expect("tempdir");
    let root = staging_root(&dir);
    let ws = WriteSerialize::with_staging_root(&root);

    let backing = dir.path().join("nested").join("file.txt");
    let path = ws.exdev_staging_path(&backing);

    assert_eq!(path.parent(), backing.parent(), "EXDEV staging must sit in the dest dir");
    assert_ne!(path.parent(), Some(root.as_path()), "not in the shared staging root");
    let name = path.file_name().expect("name").to_string_lossy();
    assert!(name.starts_with(".sharecli-staging-"), "unique staging name, got {name}");
}
