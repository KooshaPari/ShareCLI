// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Criterion microbench: Prometheus text exposition render.
//!
//! Target (draft SLO): `prometheus_render` p95 < 500 µs for 32 synthetic processes.
//! See docs/ops/SLO.md § Bench-linked targets and docs/eval/REPRO.md.

use std::collections::HashMap;
use std::hint::black_box;
use std::time::Instant;

use criterion::{criterion_group, criterion_main, Criterion};
use sharecli::commands::serve::render_prometheus_metrics;
use sharecli::health_check::HealthStatus;
use sharecli::runtime::{ProcState, ProcessInfo};

fn sample_processes(n: usize) -> Vec<ProcessInfo> {
    (0..n)
        .map(|i| ProcessInfo {
            pid: 1000 + i as u32,
            name: format!("proc-{i}"),
            cmd: vec!["echo".into(), format!("{i}")],
            memory_mb: (i as u64 % 64) + 1,
            start_time: 1_700_000_000,
            cpu_percent: 0.0,
            project: Some("bench".into()),
            harness: Some("cargo".into()),
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        })
        .collect()
}

fn sample_health(n: usize) -> HashMap<String, HealthStatus> {
    (0..n)
        .map(|i| {
            (
                format!("proc-{i}"),
                HealthStatus {
                    healthy: i % 7 != 0,
                    last_check: Instant::now(),
                    consecutive_failures: if i % 7 == 0 { 3 } else { 0 },
                    last_error: None,
                },
            )
        })
        .collect()
}

fn prometheus_render(c: &mut Criterion) {
    let processes = sample_processes(32);
    let health = sample_health(32);

    c.bench_function("prometheus_render_32", |b| {
        b.iter(|| {
            let out = render_prometheus_metrics(black_box(&processes), black_box(&health));
            black_box(out.len())
        });
    });
}

criterion_group!(benches, prometheus_render);
criterion_main!(benches);
