//! FR: FR-003
//! T-810 coordination + agent policy lift - CommandLockStore, PriorityTaskQueue, PauseCode.

use std::path::PathBuf;

use sharecli::agent_call_policy::{AgentCallPolicy, PauseCode};
use sharecli::coordination::{CommandLockStore, LockStatus, PriorityTaskQueue, QueuePriority};

/// FR-003 — CommandLockStore lifecycle: acquire, re-acquire, contention, release, get, list.
#[test]
fn fr003_command_lock_store_lifecycle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("locks.json");
    let store = CommandLockStore::new(&path);

    // Fresh store empty
    assert!(store.list_all().expect("list empty").is_empty());
    assert!(store.get("missing").expect("get missing").is_none());
    assert!(store.release("missing", 1).is_err());

    // Acquire new lock
    let lock = store.acquire("hash-1", 100, Some("/tmp/out-1")).expect("acquire 1");
    assert_eq!(lock.cmd_hash, "hash-1");
    assert_eq!(lock.pid, 100);
    assert_eq!(lock.status, LockStatus::Locked);
    assert_eq!(lock.output_path.as_deref(), Some("/tmp/out-1"));
    assert!(lock.start_time.is_some());
    assert!(lock.is_locked());
    // Unlocked variant is not locked
    let unlocked = sharecli::coordination::CommandLock {
        cmd_hash: "x".into(),
        pid: 0,
        status: LockStatus::Unlocked,
        output_path: None,
        start_time: None,
    };
    assert!(!unlocked.is_locked());

    // get and list_all
    let fetched = store.get("hash-1").expect("get").expect("some");
    assert_eq!(fetched.pid, 100);
    assert_eq!(store.list_all().expect("list").len(), 1);

    // Re-acquire same hash with same pid updates output_path
    let lock2 = store.acquire("hash-1", 100, Some("/tmp/out-2")).expect("re-acquire same pid");
    assert_eq!(lock2.output_path.as_deref(), Some("/tmp/out-2"));

    // Contention: different pid while locked -> error
    let err = store.acquire("hash-1", 200, None).unwrap_err();
    assert!(err.to_string().contains("already locked"));

    // Wrong pid release -> error
    let err = store.release("hash-1", 999).unwrap_err();
    assert!(err.to_string().contains("cannot release"));

    // Correct release
    store.release("hash-1", 100).expect("release");
    let after = store.get("hash-1").expect("get after").expect("some");
    assert_eq!(after.status, LockStatus::Unlocked);
    assert_eq!(after.pid, 0);
    assert!(after.start_time.is_none());
    assert!(!after.is_locked());

    // After release, different pid can acquire
    let lock3 = store.acquire("hash-1", 200, None).expect("acquire after release");
    assert_eq!(lock3.pid, 200);
    assert_eq!(lock3.status, LockStatus::Locked);

    // Second independent hash
    store.acquire("hash-2", 300, None).expect("acquire hash-2");
    assert_eq!(store.list_all().expect("list 2").len(), 2);

    // Persistence: new store instance reads same file
    let store2 = CommandLockStore::new(&path);
    assert_eq!(store2.list_all().expect("list persisted").len(), 2);
}

/// FR-003 — PriorityTaskQueue ordering, peek, dequeue, len, clear, persistence.
#[test]
fn fr003_priority_task_queue_ordering_and_lifecycle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("nested").join("queue.json");
    let queue = PriorityTaskQueue::new(&path);

    assert!(queue.is_empty().expect("empty"));
    assert_eq!(queue.len().expect("len 0"), 0);
    assert!(queue.peek().expect("peek empty").is_none());
    assert!(queue.dequeue().expect("dequeue empty").is_none());
    assert!(queue.list_all().expect("list empty").is_empty());

    // Enqueue in non-priority order; queue sorts by Critical(0) < High(1) < Normal(2) < Low(3)
    let low = queue.enqueue("cmd-low", QueuePriority::Low).expect("enqueue low");
    assert_eq!(low.priority, QueuePriority::Low);
    let normal = queue.enqueue("cmd-normal", QueuePriority::Normal).expect("enqueue normal");
    let critical =
        queue.enqueue("cmd-critical", QueuePriority::Critical).expect("enqueue critical");
    let high = queue.enqueue("cmd-high", QueuePriority::High).expect("enqueue high");

    assert_eq!(queue.len().expect("len 4"), 4);
    assert!(!queue.is_empty().expect("not empty"));

    // Persistence via second instance - same file, peek highest priority
    let queue2 = PriorityTaskQueue::new(&path);
    assert_eq!(queue2.len().expect("len persisted 4"), 4);
    let peek2 = queue2.peek().expect("peek persisted").expect("some");
    assert_eq!(peek2.priority, QueuePriority::Critical);

    // list_all sorted
    let listed = queue.list_all().expect("list sorted");
    assert_eq!(listed[0].priority, QueuePriority::Critical);
    assert_eq!(listed[1].priority, QueuePriority::High);
    assert_eq!(listed[2].priority, QueuePriority::Normal);
    assert_eq!(listed[3].priority, QueuePriority::Low);
    assert_eq!(listed[0].command, critical.command);

    // peek does not remove
    let peeked = queue.peek().expect("peek").expect("some");
    assert_eq!(peeked.priority, QueuePriority::Critical);
    assert_eq!(queue.len().expect("len still 4"), 4);

    // dequeue returns in priority order and marks Dequeued
    let d1 = queue.dequeue().expect("dequeue 1").expect("some");
    assert_eq!(d1.command, "cmd-critical");
    assert_eq!(d1.status, sharecli::coordination::TaskStatus::Dequeued);

    let d2 = queue.dequeue().expect("dequeue 2").expect("some");
    assert_eq!(d2.priority, QueuePriority::High);

    let d3 = queue.dequeue().expect("dequeue 3").expect("some");
    assert_eq!(d3.priority, QueuePriority::Normal);

    let d4 = queue.dequeue().expect("dequeue 4").expect("some");
    assert_eq!(d4.priority, QueuePriority::Low);

    assert!(queue.is_empty().expect("empty after dequeue"));

    // Enqueue after drain and clear
    queue.enqueue("cmd-a", QueuePriority::Normal).expect("enqueue a");
    queue.enqueue("cmd-b", QueuePriority::High).expect("enqueue b");
    assert_eq!(queue.len().expect("len 2"), 2);
    queue.clear().expect("clear");
    assert!(queue.is_empty().expect("empty after clear"));
    assert!(queue.list_all().expect("list after clear").is_empty());

    // Suppress unused warnings by referencing enqueued ids
    assert!(!low.id.is_empty());
    assert!(!normal.id.is_empty());
    assert!(!high.id.is_empty());
    let _ = path;
}

/// FR-003 — AgentCallPolicy PauseCode matrix and grep->rg normalization.
#[test]
fn fr003_agent_call_policy_pause_codes_and_normalization() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("mkdir proj");

    // Hazardous root takes precedence over all other limits
    let policy = AgentCallPolicy::new(root.clone())
        .with_thermal_headroom(false)
        .with_project_limit(0)
        .with_build_slots(0);
    let dec = policy.admit("ls /");
    assert_eq!(dec.pause_code(), Some(PauseCode::HazardousRoot));
    assert_eq!(dec.resume_condition(), Some("use a path inside the project root"));
    assert!(dec.command().contains('/'));
    assert_eq!(dec.deadline().as_secs(), 30);

    let dec = policy.admit("cat /tmp");
    assert_eq!(dec.pause_code(), Some(PauseCode::HazardousRoot));

    // Thermal pause when not hazardous
    let policy = AgentCallPolicy::new(root.clone()).with_thermal_headroom(false);
    let dec = policy.admit("echo hello");
    assert_eq!(dec.pause_code(), Some(PauseCode::Thermal));
    assert_eq!(dec.resume_condition(), Some("wait for thermal headroom"));

    // Project limit pause
    let policy = AgentCallPolicy::new(root.clone()).with_project_limit(1);
    let d1 = policy.admit("echo first");
    assert_eq!(d1.pause_code(), None);
    assert_eq!(d1.command(), "echo first");
    assert_eq!(d1.deadline().as_secs(), 30);
    assert!(d1.resume_condition().is_none());

    let d2 = policy.admit("echo second");
    assert_eq!(d2.pause_code(), Some(PauseCode::ProjectLimit));
    assert_eq!(d2.resume_condition(), Some("wait for an active project call to finish"));

    // Build slot pause — only for cargo/make/just
    let policy = AgentCallPolicy::new(root.clone()).with_build_slots(1);
    let b1 = policy.admit("cargo build");
    assert_eq!(b1.pause_code(), None);
    let b2 = policy.admit("cargo test");
    assert_eq!(b2.pause_code(), Some(PauseCode::BuildSlot));
    assert_eq!(b2.resume_condition(), Some("wait for an available build slot"));
    // Non-build still admitted even when build slots exhausted (but counts toward project limit)
    let policy2 = AgentCallPolicy::new(root.clone()).with_build_slots(0);
    let nb = policy2.admit("echo not-a-build");
    assert_eq!(nb.pause_code(), None);
    let b_blocked = policy2.admit("make check");
    assert_eq!(b_blocked.pause_code(), Some(PauseCode::BuildSlot));
    let b_blocked2 = policy2.admit("just build");
    assert_eq!(b_blocked2.pause_code(), Some(PauseCode::BuildSlot));

    // Normalization: grep -r with dot target rewrites to rg with project root
    let policy = AgentCallPolicy::new(root.clone());
    let dec = policy.admit("grep -r hello .");
    assert!(dec.command().starts_with("rg "), "expected rg rewrite, got {}", dec.command());
    assert!(dec.command().contains("hello"));
    assert!(dec.command().contains(root.to_string_lossy().as_ref()));
    assert_eq!(dec.pause_code(), None);

    // grep -R (capital) also triggers
    let dec = policy.admit("grep -R pattern .");
    assert!(dec.command().starts_with("rg "));

    // egrep variant
    let dec = policy.admit("egrep -r pattern .");
    assert!(dec.command().starts_with("rg "));

    // Without recursive flag, no rewrite
    let dec = policy.admit("grep hello .");
    assert_eq!(dec.command(), "grep hello .");

    // Explicit target path preserved
    let dec = policy.admit("grep -r pattern src");
    assert!(dec.command().ends_with(" src"));

    // Empty command passthrough
    let dec = policy.admit("");
    assert_eq!(dec.command(), "");
}
