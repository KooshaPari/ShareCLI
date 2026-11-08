// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//!
//! C01 climb-2 — ipc CoalesceCache / SlotQueue / nocache edge coverage.

use std::time::Duration;

use sharecli_ipc::{
    has_nocache_arg, parse_nocache_args_csv, resolve_operator_queue_priority,
    should_bypass_coalesce, CachedResult, CoalesceCache, CommandKey, QueuePriority, SlotQueue,
    DEFAULT_NOCACHE_ARGS,
};
use tempfile::TempDir;

/// FR-003 / C01 — CoalesceCache miss/store/hit + TTL expiry path.
#[test]
fn fr003_coalesce_cache_store_hit_and_ttl_miss() {
    let dir = TempDir::new().expect("tempdir");
    let cache = CoalesceCache::with_ttl(dir.path(), Duration::from_millis(50));
    assert_eq!(cache.ttl(), Duration::from_millis(50));

    let key = CommandKey("climb2-key-1".into());
    assert!(cache.lookup(&key).expect("lookup miss").is_none());

    cache
        .store(&key, &CachedResult { exit_code: 0, stdout: b"ok".to_vec(), stderr: Vec::new() })
        .expect("store");
    let hit = cache.lookup(&key).expect("lookup hit").expect("present");
    assert_eq!(hit.exit_code, 0);
    assert_eq!(hit.stdout, b"ok");

    std::thread::sleep(Duration::from_millis(80));
    assert!(cache.lookup(&key).expect("ttl miss").is_none());
}

/// FR-003 / C01 — CoalesceCache with_lock runs once.
#[test]
fn fr003_coalesce_cache_with_lock_single_flight() {
    let dir = TempDir::new().expect("tempdir");
    let cache =
        CoalesceCache::with_options(dir.path(), Duration::from_secs(60), Duration::from_millis(10));
    let key = CommandKey("climb2-lock-key".into());
    let mut runs = 0u32;
    let out = cache
        .with_lock(&key, || {
            runs += 1;
            Ok(CachedResult { exit_code: 0, stdout: b"once".to_vec(), stderr: Vec::new() })
        })
        .expect("with_lock");
    assert_eq!(runs, 1);
    assert_eq!(out.stdout, b"once");
    let hit = cache.lookup(&key).expect("lookup").expect("cached");
    assert_eq!(hit.stdout, b"once");
}

/// FR-003 / C01 — SlotQueue with_slot + priority parsing edges.
#[test]
fn fr003_slot_queue_and_priority_edges() {
    let dir = TempDir::new().expect("tempdir");
    let q = SlotQueue::new(dir.path().join("q"), 2);
    assert_eq!(q.max_concurrent(), 2);
    assert!(q.root().ends_with("q"));

    let v = q.with_slot("lane-a", QueuePriority::Normal, || Ok(99u8)).expect("slot");
    assert_eq!(v, 99);

    assert_eq!(QueuePriority::parse("high"), QueuePriority::High);
    assert_eq!(QueuePriority::parse("low"), QueuePriority::Low);
    assert_eq!(QueuePriority::parse("nope"), QueuePriority::Normal);
    assert_eq!(resolve_operator_queue_priority(Some("high")), QueuePriority::High);
    assert_eq!(resolve_operator_queue_priority(None), QueuePriority::Normal);
}

/// FR-003 / C01 — nocache argv detection + CSV parse.
#[test]
fn fr003_nocache_argv_and_csv_parse() {
    let argv = ["cargo", "fmt", "--", "--check"];
    assert!(!should_bypass_coalesce(&argv, DEFAULT_NOCACHE_ARGS));

    let mutating = ["ruff", "check", "--fix"];
    assert!(should_bypass_coalesce(&mutating, DEFAULT_NOCACHE_ARGS));
    assert!(has_nocache_arg(&mutating, DEFAULT_NOCACHE_ARGS));

    let custom = parse_nocache_args_csv("--fix,--force,  --write ");
    assert!(custom.iter().any(|s| s == "--fix"));
    assert!(custom.iter().any(|s| s == "--write"));
}
