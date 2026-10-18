// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 / C01 — coordination lock and priority queue coverage lift.
//!
//! FR: FR-003

use sharecli::coordination::{
    CommandLockStore, LockStatus, PriorityTaskQueue, QueuePriority, TaskStatus,
};

#[test]
fn command_lock_acquire_release_and_reacquire_by_same_pid() {
    let temp = tempfile::tempdir().unwrap();
    let store = CommandLockStore::new(temp.path().join("locks.json"));

    let first = store.acquire("cmd-hash", 1234, Some("out.log")).unwrap();
    assert_eq!(first.cmd_hash, "cmd-hash");
    assert_eq!(first.pid, 1234);
    assert_eq!(first.status, LockStatus::Locked);
    assert!(first.is_locked());

    let second = store.acquire("cmd-hash", 1234, Some("new.log")).unwrap();
    assert_eq!(second.output_path.as_deref(), Some("new.log"));
    assert!(second.is_locked());

    store.release("cmd-hash", 1234).unwrap();
    let released = store.get("cmd-hash").unwrap().unwrap();
    assert_eq!(released.pid, 0);
    assert_eq!(released.status, LockStatus::Unlocked);
    assert!(!released.is_locked());

    let reacquired = store.acquire("cmd-hash", 1234, None).unwrap();
    assert!(reacquired.is_locked());
}

#[test]
fn command_lock_rejects_other_pid_until_owner_releases() {
    let temp = tempfile::tempdir().unwrap();
    let store = CommandLockStore::new(temp.path().join("locks.json"));

    store.acquire("cmd-hash", 1111, None).unwrap();

    let err = store.acquire("cmd-hash", 2222, None).unwrap_err();
    assert!(err.to_string().contains("already locked"));

    let release_err = store.release("cmd-hash", 2222).unwrap_err();
    assert!(release_err.to_string().contains("cannot release"));
}

#[test]
fn priority_queue_lists_and_dequeues_by_priority() {
    let temp = tempfile::tempdir().unwrap();
    let queue = PriorityTaskQueue::new(temp.path().join("queue.json"));

    queue.enqueue("low", QueuePriority::Low).unwrap();
    queue.enqueue("normal", QueuePriority::Normal).unwrap();
    queue.enqueue("critical", QueuePriority::Critical).unwrap();
    queue.enqueue("high", QueuePriority::High).unwrap();

    let listed = queue.list_all().unwrap();
    let commands: Vec<_> = listed.iter().map(|item| item.command.as_str()).collect();
    assert_eq!(commands, ["critical", "high", "normal", "low"]);
    assert!(listed.iter().all(|item| item.status == TaskStatus::Pending));

    let next = queue.dequeue().unwrap().unwrap();
    assert_eq!(next.command, "critical");
    assert_eq!(next.status, TaskStatus::Dequeued);

    let remaining = queue.list_all().unwrap();
    let commands: Vec<_> = remaining.iter().map(|item| item.command.as_str()).collect();
    assert_eq!(commands, ["high", "normal", "low"]);
}

#[test]
fn priority_queue_peek_dequeue_empty_and_clear() {
    let temp = tempfile::tempdir().unwrap();
    let queue = PriorityTaskQueue::new(temp.path().join("queue.json"));

    assert!(queue.is_empty().unwrap());
    assert_eq!(queue.len().unwrap(), 0);
    assert!(queue.peek().unwrap().is_none());
    assert!(queue.dequeue().unwrap().is_none());

    queue.enqueue("only", QueuePriority::Normal).unwrap();
    assert_eq!(queue.peek().unwrap().unwrap().command, "only");
    assert_eq!(queue.len().unwrap(), 1);

    queue.clear().unwrap();
    assert!(queue.is_empty().unwrap());
    assert!(queue.list_all().unwrap().is_empty());
}

#[test]
fn command_lock_store_lists_all_entries() {
    let temp = tempfile::tempdir().unwrap();
    let store = CommandLockStore::new(temp.path().join("locks.json"));

    store.acquire("alpha", 100, None).unwrap();
    store.acquire("beta", 200, Some("beta.log")).unwrap();

    let all = store.list_all().unwrap();
    assert_eq!(all.len(), 2);
    let hashes: Vec<_> = all.iter().map(|lock| lock.cmd_hash.as_str()).collect();
    assert!(hashes.contains(&"alpha"));
    assert!(hashes.contains(&"beta"));
}

#[test]
fn command_lock_rejects_invalid_json_on_disk() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("locks.json");
    std::fs::write(&path, "not-json").unwrap();
    let store = CommandLockStore::new(&path);
    let err = store.list_all().unwrap_err();
    assert!(err.to_string().contains("failed to parse"));
}
