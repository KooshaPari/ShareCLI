// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003

//! FR-003 / C01 — InterceptFs no-mount helper coverage (Linux/macOS).
//!
//! Exercises the high-line InterceptFs API surface that does not require a live
//! FUSE mount: exists/neg-dentry, read coalesce, CoW stage/commit/discard, and
//! create/write helpers. Counted by broad-workspace llvm-cov on ubuntu CI.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::Path;

use sharecli_fuse::{InterceptFs, InterceptFsOptions};
use tempfile::TempDir;

/// FR-003 / C01 — options wiring + exists/neg-dentry + read coalesce path.
#[test]
fn fr003_intercept_fs_exists_and_read_coalesce() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("hello.txt"), b"hello-world").expect("write");

    let fs = InterceptFs::with_options(
        &backing,
        InterceptFsOptions {
            session_id: "cov-sess".into(),
            cow: true,
            cow_dir: Some(dir.path().join("cow")),
            agent: Some("agent-cov".into()),
            serialize: true,
            agents_conf: None,
        },
    );

    assert_eq!(fs.session_id(), "cov-sess");
    assert!(fs.cow_enabled());
    assert_eq!(fs.default_agent(), "agent-cov");
    assert!(fs.serialize_writes());
    assert!(fs.agents_conf().is_none());
    assert_eq!(fs.backing(), backing.as_path());

    assert!(fs.exists_rel(Path::new("hello.txt")).expect("exists"));
    assert!(!fs.exists_rel(Path::new("missing.txt")).expect("missing"));
    // Second miss should hit negative-dentry cache.
    assert!(!fs.exists_rel(Path::new("missing.txt")).expect("neg hit"));
    let neg = fs.neg_dentry_meters();
    assert!(neg.misses >= 1 || neg.hits >= 1);

    let bytes = fs.read_coalesced_rel(Path::new("hello.txt")).expect("read");
    assert_eq!(bytes, b"hello-world");
    let _ = fs.read_coalesced_rel(Path::new("hello.txt")).expect("read hit");
    let meters = fs.cache_meters();
    assert!(meters.hits + meters.misses >= 1);

    fs.invalidate_neg_rel(Path::new("missing.txt"));
}

/// FR-003 / C01 — CoW stage/commit/discard + create/write helpers.
#[test]
fn fr003_intercept_fs_cow_stage_commit_and_write() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("f.txt"), b"old").expect("seed");

    let fs = InterceptFs::with_options(
        &backing,
        InterceptFsOptions {
            session_id: "write-sess".into(),
            cow: true,
            cow_dir: Some(dir.path().join("cow")),
            agent: None,
            serialize: true,
            agents_conf: None,
        },
    );

    fs.stage_rel(Path::new("f.txt"), b"staged").expect("stage");
    let pending = fs.pending_rel_paths().expect("pending");
    assert!(pending.iter().any(|p| p.ends_with("f.txt")));
    let by_agent = fs.pending_by_agent().expect("by agent");
    assert!(!by_agent.is_empty());

    fs.commit_rel(Path::new("f.txt")).expect("commit");
    assert_eq!(fs::read(backing.join("f.txt")).expect("read"), b"staged");

    fs.stage_rel_for_agent(Some("other"), Path::new("f.txt"), b"other").expect("stage other");
    fs.discard_rel_for_agent(Some("other"), Path::new("f.txt")).expect("discard");
    assert_eq!(fs::read(backing.join("f.txt")).expect("read"), b"staged");

    fs.stage_rel(Path::new("f.txt"), b"again").expect("stage again");
    let committed = fs.commit_all_for_agent(None).expect("commit all");
    assert!(!committed.is_empty());

    fs.stage_rel(Path::new("f.txt"), b"toss").expect("stage toss");
    let discarded = fs.discard_all_for_agent(None).expect("discard all");
    assert!(!discarded.is_empty());

    fs.create_rel(Path::new("nested/new.txt"), 0o644).expect("create");
    assert!(backing.join("nested/new.txt").is_file());
    let n = fs.write_rel(Path::new("nested/new.txt"), 0, b"payload").expect("write");
    assert_eq!(n, 7);
    assert_eq!(fs::read(backing.join("nested/new.txt")).expect("read"), b"payload");
}

/// FR-003 / C01 — empty session id falls back; InterceptFs::new works.
#[test]
fn fr003_intercept_fs_defaults_and_empty_session() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");

    let plain = InterceptFs::new(&backing);
    assert!(!plain.session_id().is_empty());
    assert!(!plain.cow_enabled());

    let empty_sess = InterceptFs::with_options(
        &backing,
        InterceptFsOptions { session_id: String::new(), ..InterceptFsOptions::default() },
    );
    assert!(!empty_sess.session_id().is_empty());
}
