// FR: FR-008 mature-recovery queue identity safety
//! Mature-recovery adversarial control for queue ownership.
//!
//! A stale ticket can name a PID that is currently live again. PID liveness
//! alone therefore cannot establish that the current process owns the old
//! waiter claim. No product remediation is included.

#![cfg(unix)]

use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sharecli_ipc::{QueuePriority, SlotQueue};
use tempfile::TempDir;

#[test]
fn stale_ticket_with_reused_live_pid_must_not_block_new_waiter() {
    let dir = TempDir::new().expect("fixture");
    let q = SlotQueue::with_options(
        dir.path(),
        1,
        Duration::from_millis(500),
        Duration::from_millis(10),
    );

    let lane = "pid-reuse";
    let waiting = dir.path().join(format!("{lane}.waiting"));
    fs::create_dir_all(&waiting).expect("waiting dir");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();

    // This file simulates an old waiter whose PID has since been reused by
    // the current process. It looks live to any PID-only check but has no
    // valid generation/lease relationship to this queue attempt.
    let stale_reused_pid = format!("00.{now}.{}.1", std::process::id());
    fs::write(waiting.join(stale_reused_pid), b"0\n").expect("stale ticket");

    let result = q.with_slot(lane, QueuePriority::Normal, || Ok::<_, anyhow::Error>(()));

    assert!(
        result.is_ok(),
        "a stale ticket sharing a live/reused PID blocked progress; ownership needs generation/lease identity"
    );
}
