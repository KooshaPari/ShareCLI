// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003

//! FR-003 / C01 — CowMountHandle edge-path coverage (all supported OS).

use std::fs;
use std::path::Path;

use sharecli_fuse::{CowMountHandle, InterceptFsOptions};
use tempfile::TempDir;

/// FR-003 / C01 — CoW-disabled staging errors; discard + empty session fallback.
#[test]
fn fr003_cow_handle_disabled_and_discard_paths() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("back");
    fs::create_dir_all(&backing).expect("backing");
    fs::write(backing.join("f.txt"), b"seed").expect("seed");

    let disabled = CowMountHandle::from_options(
        &backing,
        &InterceptFsOptions {
            session_id: String::new(),
            cow: false,
            cow_dir: Some(dir.path().join("cow-off")),
            agent: None,
            serialize: false,
            agents_conf: None,
        },
    );
    assert!(!disabled.cow_enabled());
    assert!(!disabled.session_id().is_empty());
    assert_eq!(disabled.backing(), backing.as_path());
    assert!(disabled
        .stage_rel_for_agent(None, Path::new("f.txt"), b"x")
        .unwrap_err()
        .to_string()
        .contains("CoW"));

    let enabled = CowMountHandle::from_options(
        &backing,
        &InterceptFsOptions {
            session_id: "sess-discard".into(),
            cow: true,
            cow_dir: Some(dir.path().join("cow-on")),
            agent: Some("agent-b".into()),
            serialize: true,
            agents_conf: None,
        },
    );
    assert_eq!(enabled.default_agent(), "agent-b");
    assert_eq!(enabled.cow_root(), dir.path().join("cow-on").as_path());

    enabled.stage_rel_for_agent(Some("agent-b"), Path::new("f.txt"), b"pending").expect("stage");
    let pending = enabled.pending_rel_paths_for_agent(Some("agent-b")).expect("pending");
    assert!(pending.iter().any(|p| p.ends_with("f.txt")));
    let grouped = enabled.pending_by_agent().expect("grouped");
    assert!(!grouped.is_empty());

    enabled.discard_rel_for_agent(Some("agent-b"), Path::new("f.txt")).expect("discard");
    assert_eq!(fs::read(backing.join("f.txt")).expect("unchanged"), b"seed");

    enabled
        .stage_rel_for_agent(Some("agent-b"), Path::new("f.txt"), b"again")
        .expect("stage again");
    let discarded = enabled.discard_all_for_agent(Some("agent-b")).expect("discard all");
    assert!(!discarded.is_empty());

    enabled
        .stage_rel_for_agent(Some("agent-b"), Path::new("f.txt"), b"final")
        .expect("stage final");
    let committed = enabled.commit_all_for_agent(Some("agent-b")).expect("commit all");
    assert!(!committed.is_empty());
    assert_eq!(fs::read(backing.join("f.txt")).expect("committed"), b"final");

    let locked =
        enabled.with_locked_path(Some("agent-b"), &backing.join("f.txt"), || 42u32).expect("lock");
    assert_eq!(locked, 42);
}
