// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Criterion microbench: TOML Config deserialize (hot path for serve/reload).
//!
//! Target (draft SLO): `config_parse` p95 < 1 ms for the default document.
//! See docs/ops/SLO.md § Bench-linked targets and docs/eval/REPRO.md.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use sharecli::config::Config;

fn config_parse(c: &mut Criterion) {
    let sample = toml::to_string_pretty(&Config::default()).expect("serialize default config");

    c.bench_function("config_toml_from_str", |b| {
        b.iter(|| {
            let cfg: Config = toml::from_str(black_box(sample.as_str())).expect("sample config");
            black_box(cfg.pool.max_per_type)
        });
    });
}

criterion_group!(benches, config_parse);
criterion_main!(benches);
