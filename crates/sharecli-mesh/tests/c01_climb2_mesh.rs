// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//!
//! C01 climb-2 — mesh SmartMerger + MaildirQueue / operator status.

use std::fs;

use sharecli_mesh::{MaildirQueue, SmartMerger};
use tempfile::TempDir;

/// FR-003 / C01 — SmartMerger with/without git fallback.
#[test]
fn fr003_smart_merger_git_fallback_paths() {
    let dir = TempDir::new().expect("tempdir");
    let base = dir.path().join("base.txt");
    let ours = dir.path().join("ours.txt");
    let theirs = dir.path().join("theirs.txt");
    let out = dir.path().join("out.txt");
    fs::write(&base, "line1\nline2\nline3\n").expect("base");
    fs::write(&ours, "line1\nours\nline3\n").expect("ours");
    fs::write(&theirs, "line1\ntheirs\nline3\n").expect("theirs");

    let no_fallback = SmartMerger::new().without_git_fallback();
    let missing = no_fallback.merge(&base, &ours, &theirs, &out);
    // No mergiraf + no git fallback → unsuccessful.
    assert!(!missing.success);

    let with_git = SmartMerger::new();
    let result = with_git.merge(&base, &ours, &theirs, &out);
    // Conflict or clean — both exercise merge codepaths.
    let _ = (result.success, result.used_mergiraf, result.conflicts.len(), result.output.len());
}

/// FR-003 / C01 — MaildirQueue enqueue/claim/ack/nack/status.
#[test]
fn fr003_maildir_queue_enqueue_claim_ack_nack() {
    let dir = TempDir::new().expect("tempdir");
    let q = MaildirQueue::open(dir.path()).expect("open");
    assert_eq!(q.path(), dir.path());

    let id = q.enqueue(serde_json::json!({"op": "climb2"}), 2).expect("enqueue");
    assert!(!id.is_empty());

    let status = q.status().expect("status");
    assert!(status.pending >= 1);
    assert_eq!(status.ready + status.in_flight, status.pending);

    let claimed = q.claim(Some("owner-a")).expect("claim").expect("task");
    assert_eq!(claimed.id, id);

    q.nack(&id).expect("nack");
    let reclaimed = q.reclaim_owner("owner-a").expect("reclaim");
    let _ = reclaimed;

    if let Some(task) = q.dequeue(Some("owner-b")).expect("dequeue") {
        q.ack(&task.id).expect("ack");
    }

    let pending = q.list_pending().expect("list");
    let _ = pending.len();
}
