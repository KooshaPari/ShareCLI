// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L7 — loom model of ProcessPool pid registry (FR:003 · T-670).

#![cfg(loom)]

use std::sync::Arc;

use loom::model;
use loom::thread;
use sharecli_sync::{PoolIndex, RelaxedCounter};

#[test]
fn concurrent_unique_inserts() {
    model(|| {
        let idx = Arc::new(PoolIndex::new());
        let a = Arc::clone(&idx);
        let b = Arc::clone(&idx);

        let t1 = thread::spawn(move || {
            assert!(a.insert(1, "a"));
            assert!(a.insert(2, "b"));
        });
        let t2 = thread::spawn(move || {
            assert!(b.insert(3, "c"));
            assert!(b.insert(4, "d"));
        });

        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(idx.count(), 4);
    });
}

#[test]
fn insert_remove_balanced() {
    model(|| {
        let idx = Arc::new(PoolIndex::new());
        idx.insert(10, "proc");
        let a = Arc::clone(&idx);
        let b = Arc::clone(&idx);

        let t1 = thread::spawn(move || {
            let _ = a.remove(10);
        });
        let t2 = thread::spawn(move || {
            let _ = b.insert(11, "other");
        });

        t1.join().unwrap();
        t2.join().unwrap();
        assert!(idx.count() <= 2);
    });
}

#[test]
fn relaxed_counter_concurrent_inc() {
    model(|| {
        let counter = Arc::new(RelaxedCounter::new());
        let c1 = Arc::clone(&counter);
        let c2 = Arc::clone(&counter);

        let t1 = thread::spawn(move || {
            for _ in 0..2 {
                c1.inc();
            }
        });
        let t2 = thread::spawn(move || {
            for _ in 0..2 {
                c2.inc();
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
        assert_eq!(counter.get(), 4);
    });
}
