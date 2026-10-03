// FR: FR-008 / SC-WP-B03. Existing queue contracts retained; manually created
// positive waiter fixtures now hold a real lease rather than impersonating one.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use tempfile::TempDir;

fn queue(root: &Path, slots: usize) -> SlotQueue {
    SlotQueue::with_options(root, slots, Duration::from_secs(5), Duration::from_millis(5))
}

fn wait_flag(flag: &AtomicBool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !flag.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "fixture synchronization timed out");
        thread::sleep(Duration::from_millis(2));
    }
}

fn owned_ticket(path: &Path) -> fs::File {
    let mut file = fs::OpenOptions::new().create_new(true).read(true).write(true).open(path).unwrap();
    FileExt::lock_exclusive(&file).unwrap();
    writeln!(file, "lease fixture").unwrap();
    file
}

#[test]
fn priority_parse() {
    assert_eq!(QueuePriority::parse("critical"), QueuePriority::Critical);
    assert_eq!(QueuePriority::parse("HIGH"), QueuePriority::High);
    assert_eq!(QueuePriority::parse("unknown"), QueuePriority::Normal);
}

#[test]
fn priority_parse_case_insensitive() {
    for (name, expected) in [
        ("Critical", QueuePriority::Critical), ("LOW", QueuePriority::Low),
        ("Background", QueuePriority::Background), ("NORMAL", QueuePriority::Normal),
        ("  High  ", QueuePriority::High),
    ] {
        assert_eq!(QueuePriority::parse(name), expected);
    }
}

#[test]
fn priority_as_u8() {
    assert_eq!(QueuePriority::Critical.as_u8(), 0);
    assert_eq!(QueuePriority::High.as_u8(), 1);
    assert_eq!(QueuePriority::Normal.as_u8(), 2);
    assert_eq!(QueuePriority::Low.as_u8(), 3);
    assert_eq!(QueuePriority::Background.as_u8(), 4);
}

#[test]
fn priority_ordering() {
    assert!(QueuePriority::Critical < QueuePriority::High);
    assert!(QueuePriority::High < QueuePriority::Normal);
    assert!(QueuePriority::Normal < QueuePriority::Low);
    assert!(QueuePriority::Low < QueuePriority::Background);
}

#[test]
fn priority_default_is_normal() {
    assert_eq!(QueuePriority::default(), QueuePriority::Normal);
}

#[test]
fn slot_queue_root_and_max_concurrent() {
    let dir = TempDir::new().unwrap();
    let q = SlotQueue::new(dir.path(), 3);
    assert_eq!(q.root(), dir.path());
    assert_eq!(q.max_concurrent(), 3);
}

#[test]
fn slot_queue_max_concurrent_min_one() {
    let dir = TempDir::new().unwrap();
    assert_eq!(SlotQueue::new(dir.path(), 0).max_concurrent(), 1);
}

#[test]
fn with_slot_returns_ok_value() {
    let dir = TempDir::new().unwrap();
    assert_eq!(queue(dir.path(), 2).with_slot("value", QueuePriority::Normal, || Ok(42)).unwrap(), 42);
}

#[test]
fn with_slot_propagates_error() {
    let dir = TempDir::new().unwrap();
    let result: Result<()> = queue(dir.path(), 2).with_slot("error", QueuePriority::Normal, || anyhow::bail!("intentional error"));
    assert!(result.unwrap_err().to_string().contains("intentional error"));
}

#[test]
fn with_slot_creates_root_dir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("nested").join("queue");
    assert!(!root.exists());
    queue(&root, 1).with_slot("lane", QueuePriority::Normal, || Ok(())).unwrap();
    assert!(root.exists());
}

fn concurrent_peak(slots: usize, lanes: &[&str]) -> u32 {
    let dir = TempDir::new().unwrap();
    let active = Arc::new(AtomicU32::new(0));
    let peak = Arc::new(AtomicU32::new(0));
    let width = if lanes.iter().all(|v| v == &lanes[0]) { slots } else { lanes.len() };
    let barrier = Arc::new(Barrier::new(width));
    let mut threads = Vec::new();
    for lane in lanes {
        let (root, lane) = (dir.path().to_path_buf(), lane.to_string());
        let (active, peak) = (Arc::clone(&active), Arc::clone(&peak));
        let barrier = Arc::clone(&barrier);
        threads.push(thread::spawn(move || {
            queue(&root, slots).with_slot(&lane, QueuePriority::Normal, || {
                let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(n, Ordering::SeqCst);
                barrier.wait();
                thread::sleep(Duration::from_millis(50));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }).unwrap();
        }));
    }
    for handle in threads { handle.join().unwrap(); }
    peak.load(Ordering::SeqCst)
}

#[test]
fn with_slot_serializes_max_one() {
    assert_eq!(concurrent_peak(1, &["ruff", "ruff", "ruff", "ruff"]), 1);
}

#[test]
fn with_slot_allows_two() {
    assert_eq!(concurrent_peak(2, &["pytest", "pytest", "pytest", "pytest"]), 2);
}

#[test]
fn with_slot_concurrent_different_lanes() {
    assert!(concurrent_peak(1, &["lane-a", "lane-b", "lane-c"]) >= 2);
}

#[test]
fn critical_dequeues_before_normal_under_contention() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    q.ensure_root().unwrap();
    let slot = fs::OpenOptions::new().create(true).write(true).truncate(false).open(q.slot_path("lane", 0)).unwrap();
    FileExt::lock_exclusive(&slot).unwrap();
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for priority in [QueuePriority::Normal, QueuePriority::Critical] {
        let root = dir.path().to_path_buf();
        let output = Arc::clone(&results);
        handles.push(thread::spawn(move || {
            queue(&root, 1).with_slot("lane", priority, || { output.lock().unwrap().push(priority); Ok(()) }).unwrap();
        }));
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let n = fs::read_dir(q.waiting_dir("lane")).map(|entries| entries.filter_map(|e| e.ok()).filter(|e| !e.file_name().to_string_lossy().starts_with('.')).count()).unwrap_or(0);
        if n == 2 { break; }
        assert!(Instant::now() < deadline, "waiter publication timed out");
        thread::sleep(Duration::from_millis(2));
    }
    drop(slot);
    for handle in handles { handle.join().unwrap(); }
    assert_eq!(*results.lock().unwrap(), vec![QueuePriority::Critical, QueuePriority::Normal]);
}

#[test]
fn with_slot_cleans_ticket_on_closure_error() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let result: Result<()> = q.with_slot("err", QueuePriority::Normal, || anyhow::bail!("intentional closure failure"));
    assert!(result.is_err());
    assert_eq!(fs::read_dir(q.waiting_dir("err")).unwrap().count(), 0);
}

#[test]
fn with_slot_cleans_ticket_on_timeout() {
    let dir = TempDir::new().unwrap();
    let q = SlotQueue::with_options(dir.path(), 1, Duration::from_millis(100), Duration::from_millis(5));
    let slot = fs::OpenOptions::new().create(true).write(true).truncate(false).open(q.slot_path("timeout", 0)).unwrap();
    FileExt::lock_exclusive(&slot).unwrap();
    assert!(q.with_slot("timeout", QueuePriority::Normal, || Ok(())).is_err());
    assert_eq!(fs::read_dir(q.waiting_dir("timeout")).unwrap().count(), 0);
}

#[test]
fn with_slot_timeout_when_all_slots_busy() {
    let dir = TempDir::new().unwrap();
    let active = Arc::new(AtomicBool::new(false));
    let ready = Arc::clone(&active);
    let root = dir.path().to_path_buf();
    let holder = thread::spawn(move || {
        queue(&root, 1).with_slot("timeout-lane", QueuePriority::Normal, || {
            ready.store(true, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(500));
            Ok(())
        }).unwrap();
    });
    wait_flag(&active);
    let q = SlotQueue::with_options(dir.path(), 1, Duration::from_millis(100), Duration::from_millis(5));
    let error = q.with_slot("timeout-lane", QueuePriority::Normal, || Ok(())).unwrap_err();
    assert!(error.to_string().contains("queue timeout"));
    holder.join().unwrap();
}

#[test]
fn ticket_priority_parses_first_field() {
    for (ticket, rank) in [("00.123.456.789", 0), ("02.123.456.789", 2), ("04.123.456.789", 4), ("bad-format", 2)] {
        assert_eq!(SlotQueue::ticket_priority(ticket), rank);
    }
}

#[test]
fn with_slot_priority_tagged_waiters() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let (guard, name) = q.enqueue_waiter("priority-lane", QueuePriority::Critical).unwrap();
    assert!(name.starts_with("00."));
    assert!(guard.path.exists());
    assert!(SlotQueue::ticket_has_lease(&guard.path).unwrap());
}

#[test]
fn effective_rank_decays_over_time() {
    for (ms, rank) in [(0, 0), (999, 0), (1_000, 1), (2_000, 2), (4_000, 4)] {
        assert_eq!(SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_millis(ms)), rank);
    }
    assert_eq!(SlotQueue::effective_rank("ignored", QueuePriority::Normal, Duration::ZERO), SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_secs(2)));
    assert!(SlotQueue::effective_rank("ignored", QueuePriority::Normal, Duration::ZERO) < SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_secs(3)));
    assert_eq!(SlotQueue::effective_rank("ignored", QueuePriority::Background, Duration::ZERO), SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_secs(4)));
}

#[test]
fn effective_rank_must_saturate_instead_of_wrapping_at_256_steps() {
    for seconds in [255, 256, 300] {
        assert_eq!(SlotQueue::effective_rank("ignored", QueuePriority::Critical, Duration::from_secs(seconds)), u8::MAX, "aging wrapped after saturation");
    }
}

#[test]
fn orphan_critical_does_not_starve_fresh_normal() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let waiting = q.waiting_dir("orphan");
    fs::create_dir_all(&waiting).unwrap();
    let old = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().saturating_sub(4);
    fs::write(waiting.join(format!("00.{old}.{}.0", std::process::id())), "0\n").unwrap();
    let start = Instant::now();
    q.with_slot("orphan", QueuePriority::Normal, || {
        assert!(start.elapsed() < Duration::from_secs(1));
        Ok(())
    }).unwrap();
}

#[test]
fn ticket_sequence_fifo_must_be_numeric_not_lexicographic() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let waiting = q.waiting_dir("sequence-fifo");
    fs::create_dir_all(&waiting).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 1;
    let pid = std::process::id();
    let earlier = format!("02.{now}.{pid}.2");
    let later = format!("02.{now}.{pid}.10");
    let _earlier_lease = owned_ticket(&waiting.join(&earlier));
    let _later_lease = owned_ticket(&waiting.join(&later));
    assert!(q.is_my_turn("sequence-fifo", QueuePriority::Normal, &earlier, Instant::now()).unwrap());
    assert!(!q.is_my_turn("sequence-fifo", QueuePriority::Normal, &later, Instant::now()).unwrap());
}

#[test]
fn fresh_dead_waiter_must_not_block_live_waiter() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let waiting = q.waiting_dir("dead-waiter");
    fs::create_dir_all(&waiting).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    fs::write(waiting.join(format!("00.{now}.{}.1", u32::MAX)), "0\n").unwrap();
    let mine = format!("02.{now}.{}.2", std::process::id());
    let _lease = owned_ticket(&waiting.join(&mine));
    assert!(q.is_my_turn("dead-waiter", QueuePriority::Normal, &mine, Instant::now()).unwrap());
}

#[test]
#[serial_test::serial]
fn resolve_operator_queue_priority_env_overrides_rule() {
    unsafe { std::env::set_var(QUEUE_PRIORITY_ENV, "critical"); }
    assert_eq!(resolve_operator_queue_priority(Some("low")), QueuePriority::Critical);
    unsafe { std::env::remove_var(QUEUE_PRIORITY_ENV); }
}

#[test]
#[serial_test::serial]
fn resolve_operator_queue_priority_rule_fallback() {
    unsafe { std::env::remove_var(QUEUE_PRIORITY_ENV); }
    assert_eq!(resolve_operator_queue_priority(Some("high")), QueuePriority::High);
    assert_eq!(resolve_operator_queue_priority(None), QueuePriority::Normal);
}

#[test]
#[serial_test::serial]
fn resolve_operator_queue_priority_empty_env_uses_rule() {
    unsafe { std::env::set_var(QUEUE_PRIORITY_ENV, "  "); }
    assert_eq!(resolve_operator_queue_priority(Some("low")), QueuePriority::Low);
    unsafe { std::env::remove_var(QUEUE_PRIORITY_ENV); }
}

#[test]
#[serial_test::serial]
fn resolve_operator_queue_priority_no_env_no_rule_is_normal() {
    unsafe { std::env::remove_var(QUEUE_PRIORITY_ENV); }
    assert_eq!(resolve_operator_queue_priority(None), QueuePriority::Normal);
}

// New ownership tests. Same-process independent opens must also conflict.
#[test]
fn lease_is_held_before_publication_and_returned_on_drop() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let (guard, name) = q.enqueue_waiter("lease", QueuePriority::Normal).unwrap();
    assert!(SlotQueue::ticket_has_lease(&guard.path).unwrap());
    let path = guard.path.clone();
    assert!(q.is_my_turn("lease", QueuePriority::Normal, &name, Instant::now()).unwrap());
    drop(guard);
    assert!(!path.exists());
}

#[test]
fn same_pid_without_lease_is_not_an_owner() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let (guard, name) = q.enqueue_waiter("lease", QueuePriority::Normal).unwrap();
    let old = q.waiting_dir("lease").join(format!("00.0.{}.0", std::process::id()));
    fs::write(&old, "0\n").unwrap();
    assert!(!SlotQueue::ticket_has_lease(&old).unwrap());
    assert!(q.is_my_turn("lease", QueuePriority::Normal, &name, Instant::now()).unwrap());
    assert!(old.exists(), "peer unlinked an unowned historical ticket");
    drop(guard);
}

#[test]
fn held_critical_lease_is_not_discarded_by_pid_hint() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let (mine, name) = q.enqueue_waiter("lease", QueuePriority::Normal).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 1;
    let peer = q.waiting_dir("lease").join(format!("00.{now}.{}.0", u32::MAX));
    let owner = owned_ticket(&peer);
    assert!(!q.is_my_turn("lease", QueuePriority::Normal, &name, Instant::now()).unwrap());
    drop(owner);
    assert!(q.is_my_turn("lease", QueuePriority::Normal, &name, Instant::now()).unwrap());
    drop(mine);
}

#[test]
fn saturated_cohort_still_has_one_fifo_winner() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let waiting = q.waiting_dir("saturated");
    fs::create_dir_all(&waiting).unwrap();
    let old = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().saturating_sub(300);
    let first = format!("00.{old}.1.2");
    let second = format!("00.{old}.1.10");
    let _first = owned_ticket(&waiting.join(&first));
    let _second = owned_ticket(&waiting.join(&second));
    let enqueued = Instant::now() - Duration::from_secs(300);
    assert!(q.is_my_turn("saturated", QueuePriority::Critical, &first, enqueued).unwrap());
    assert!(!q.is_my_turn("saturated", QueuePriority::Critical, &second, enqueued).unwrap());
}

#[test]
fn missing_own_lease_fails_closed() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let (guard, name) = q.enqueue_waiter("missing", QueuePriority::Normal).unwrap();
    fs::remove_file(&guard.path).unwrap();
    assert!(q.is_my_turn("missing", QueuePriority::Normal, &name, Instant::now()).is_err());
}

#[test]
fn closure_panic_releases_slot_and_waiter() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    let panicked = std::panic::catch_unwind(|| q.with_slot::<()>("panic", QueuePriority::Normal, || panic!("fixture")));
    assert!(panicked.is_err());
    assert_eq!(fs::read_dir(q.waiting_dir("panic")).unwrap().count(), 0);
    q.with_slot("panic", QueuePriority::Normal, || Ok(())).unwrap();
}

#[test]
fn lane_cannot_escape_queue_root() {
    let dir = TempDir::new().unwrap();
    let q = queue(dir.path(), 1);
    for lane in ["", " ", ".", "..", "../outside", "a/b", "a\\b"] {
        assert!(q.with_slot(lane, QueuePriority::Normal, || Ok(())).is_err());
    }
}

#[test]
fn shared_orphan_probes_do_not_impersonate_an_owner() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("orphan");
    fs::write(&path, "0\n").unwrap();
    let first = fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();
    FileExt::lock_shared(&first).unwrap();
    assert!(!SlotQueue::ticket_has_lease(&path).unwrap());
}

#[test]
fn publication_sequence_is_shared_by_queue_instances() {
    let dir = TempDir::new().unwrap();
    let (first, one) = queue(dir.path(), 1).enqueue_waiter("ordered", QueuePriority::Normal).unwrap();
    let (second, two) = queue(dir.path(), 1).enqueue_waiter("ordered", QueuePriority::Normal).unwrap();
    assert_eq!(SlotQueue::ticket_fifo_key(&one).0, 1);
    assert_eq!(SlotQueue::ticket_fifo_key(&two).0, 2);
    drop(first);
    drop(second);
    let (_third, three) = queue(dir.path(), 1).enqueue_waiter("ordered", QueuePriority::Normal).unwrap();
    assert_eq!(SlotQueue::ticket_fifo_key(&three).0, 3);
}

#[test]
fn torn_ordering_journal_is_not_reset_to_zero() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("torn.enqueue.lock"), "1\n2").unwrap();
    let q = queue(dir.path(), 1);
    assert!(q.with_slot("torn", QueuePriority::Normal, || Ok(())).is_err());
    assert_eq!(fs::read_dir(q.waiting_dir("torn")).unwrap().count(), 0);
}
