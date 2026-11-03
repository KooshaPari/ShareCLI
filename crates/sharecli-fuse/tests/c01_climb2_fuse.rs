// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//!
//! C01 climb-2 — fuse WriteSerialize / AgentCowStore / meter surfaces.

use std::fs;

use sharecli_fuse::{
    global_write_serialize_meters, record_commit, record_discard, record_passthrough_write,
    record_stage, AgentCowStore, WriteSerialize, WriteSerializeError, WriteSerializeMeters,
};
use tempfile::TempDir;

/// FR-003 / C01 — WriteSerialize stage/commit/discard + pending listing.
#[test]
fn fr003_write_serialize_stage_commit_discard_and_pending() {
    let dir = TempDir::new().expect("tempdir");
    let staging = dir.path().join("staging");
    let backing = dir.path().join("file.txt");
    fs::write(&backing, b"seed").expect("seed");

    let ws = WriteSerialize::with_staging_root(&staging);
    assert_eq!(ws.staging_root(), staging.as_path());
    assert!(!ws.has_pending(&backing).expect("has_pending"));

    ws.stage_bytes(&backing, b"staged").expect("stage");
    assert!(ws.has_pending(&backing).expect("pending"));
    let pending = ws.pending_backing_paths().expect("list");
    assert!(pending.iter().any(|p| p == &backing));

    let locked = ws.with_locked_path(&backing, || 7u8).expect("lock");
    assert_eq!(locked, 7);

    ws.commit_pending(&backing).expect("commit");
    assert_eq!(fs::read(&backing).expect("read"), b"staged");
    assert!(!ws.has_pending(&backing).expect("cleared"));

    ws.stage_bytes(&backing, b"again").expect("stage2");
    ws.discard_pending(&backing).expect("discard");
    assert_eq!(fs::read(&backing).expect("unchanged"), b"staged");
    assert!(matches!(ws.discard_pending(&backing), Err(WriteSerializeError::NoPending(_))));
}

/// FR-003 / C01 — WriteSerializeMeters formatting + global record helpers.
#[test]
fn fr003_write_serialize_meters_format_and_global_records() {
    let before = global_write_serialize_meters();
    record_passthrough_write();
    record_stage();
    record_commit();
    record_discard();
    let after = global_write_serialize_meters();
    assert!(after.passthrough_writes >= before.passthrough_writes + 1);
    assert!(after.stages >= before.stages + 1);
    assert!(after.commits >= before.commits + 1);
    assert!(after.discards >= before.discards + 1);

    let meters = WriteSerializeMeters { passthrough_writes: 3, stages: 2, commits: 1, discards: 1 };
    let section = meters.format_status_section();
    assert!(!section.is_empty());
}

/// FR-003 / C01 — AgentCowStore multi-agent stage/commit/discard grouping.
#[test]
fn fr003_agent_cow_store_multi_agent_pending_and_commit() {
    let dir = TempDir::new().expect("tempdir");
    let cow = dir.path().join("cow");
    let backing = dir.path().join("shared.txt");
    fs::write(&backing, b"base").expect("seed");

    let store = AgentCowStore::new(&cow, "default-agent", true);
    assert_eq!(store.default_agent(), "default-agent");
    assert!(store.serialize());
    assert_eq!(store.cow_root(), cow.as_path());

    store.stage_bytes(Some("alpha"), &backing, b"alpha-edit").expect("stage alpha");
    store.stage_bytes(Some("beta"), &backing, b"beta-edit").expect("stage beta");

    let pending_a = store.pending_for_agent(Some("alpha")).expect("pending a");
    assert!(!pending_a.is_empty());
    let grouped = store.list_agent_pending().expect("grouped");
    assert!(grouped.len() >= 2);

    store.discard_pending(Some("beta"), &backing).expect("discard beta");
    store.commit_pending(Some("alpha"), &backing).expect("commit alpha");
    assert_eq!(fs::read(&backing).expect("committed"), b"alpha-edit");

    let locked = store.with_locked_path(Some("alpha"), &backing, || 42u32).expect("lock");
    assert_eq!(locked, 42);
}

/// FR-003 / C01 — AgentCowStore commit_all / discard_all for climb-2 coverage.
#[test]
fn fr003_agent_cow_store_commit_all_discard_all() {
    let dir = TempDir::new().expect("tempdir");
    let cow = dir.path().join("cow");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"a0").expect("a");
    fs::write(&b, b"b0").expect("b");

    let store = AgentCowStore::new(&cow, "agent-x", true);
    store.stage_bytes(None, &a, b"a1").expect("stage a");
    store.stage_bytes(None, &b, b"b1").expect("stage b");
    let committed = store.commit_all_for_agent(None).expect("commit all");
    assert_eq!(committed.len(), 2);
    assert_eq!(fs::read(&a).expect("a1"), b"a1");
    assert_eq!(fs::read(&b).expect("b1"), b"b1");

    store.stage_bytes(Some("agent-x"), &a, b"a2").expect("stage a2");
    let discarded = store.discard_all_for_agent(Some("agent-x")).expect("discard all");
    assert_eq!(discarded.len(), 1);
    assert_eq!(fs::read(&a).expect("still a1"), b"a1");
}
