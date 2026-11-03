// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Criterion microbench: ProcessPool construction + empty list.
//!
//! Target (draft SLO): `pool_list` p95 < 50 ms on CI ubuntu-latest (empty pool).
//! See docs/ops/SLO.md § Bench-linked targets and docs/eval/REPRO.md.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use sharecli::runtime::ProcessPool;

fn pool_new_list(c: &mut Criterion) {
    let rt =
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");

    c.bench_function("pool_new_and_list_empty", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = ProcessPool::new();
            let list = pool.list().await;
            black_box(list.len())
        });
    });
}

criterion_group!(benches, pool_new_list);
criterion_main!(benches);
