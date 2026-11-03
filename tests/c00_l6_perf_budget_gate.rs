// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L6 / FR-003 — tight perf budgets (10% bench-gate) + profiler CI artifacts.

#[test]
fn c00_l6_baseline_ten_percent_regression() {
    let baseline = include_str!("../docs/eval/baselines/criterion-baseline.json");
    assert!(
        baseline.contains("\"default_max_regression\": 0.10"),
        "criterion baseline must pin 10% default_max_regression"
    );
}

#[test]
fn c00_l6_bench_workflow_ten_percent_gate() {
    let workflow = include_str!("../.github/workflows/bench.yml");
    assert!(workflow.contains("--threshold 0.10"), "bench.yml must use 10% gate");
    assert!(
        workflow.contains("criterion-profiler"),
        "bench.yml must upload Criterion profiler artifacts"
    );
}

#[test]
fn c00_l6_perf_budgets_doc() {
    let doc = include_str!("../docs/ops/perf-budgets.md");
    assert!(doc.contains("10%"), "perf-budgets.md must document 10% gate");
}
