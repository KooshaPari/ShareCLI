//! Positive mature-recovery control for ShareCLI local admission.
//!
//! A panicking owner must not leave the OS slot wedged. This distinguishes
//! useful RAII/file-lock behavior from the separate ticket fairness defects.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use sharecli_ipc::{QueuePriority, SlotQueue};
use tempfile::TempDir;

#[test]
fn panicking_owner_releases_slot_and_lane_remains_usable() {
    let dir = TempDir::new().expect("fixture");
    let q = SlotQueue::with_options(
        dir.path(),
        1,
        Duration::from_secs(2),
        Duration::from_millis(10),
    );

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = q.with_slot("crash-lane", QueuePriority::Normal, || -> anyhow::Result<()> {
            panic!("intentional recovery fixture panic");
        });
    }));
    assert!(panic_result.is_err(), "fixture must actually panic");

    let start = Instant::now();
    let q2 = SlotQueue::with_options(
        dir.path(),
        1,
        Duration::from_secs(2),
        Duration::from_millis(10),
    );
    q2.with_slot("crash-lane", QueuePriority::Normal, || Ok::<_, anyhow::Error>(()))
        .expect("subsequent owner must acquire after panic");

    assert!(
        start.elapsed() < Duration::from_secs(1),
        "panic must not leave the admission slot wedged"
    );
}
