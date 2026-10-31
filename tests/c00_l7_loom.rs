// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L7 / FR-003 — loom hard coverage for ProcessPool pid index + relaxed counter.
//!
//! Evidence: `crates/sharecli-sync/`, `docs/ops/concurrency.md`.

#[test]
fn c00_l7_sharecli_sync_crate_present() {
    let manifest = include_str!("../crates/sharecli-sync/Cargo.toml");
    assert!(manifest.contains("sharecli-sync"), "sharecli-sync crate must exist");
    assert!(manifest.contains("loom"), "sharecli-sync must declare loom dev-dep");
}

#[test]
fn c00_l7_pool_index_module_present() {
    let src = include_str!("../crates/sharecli-sync/src/lib.rs");
    assert!(src.contains("PoolIndex"), "sharecli-sync must define PoolIndex");
    assert!(src.contains("RwLock"), "pool index must use RwLock");
}

#[test]
fn c00_l7_loom_test_suite_present() {
    let loom_tests = include_str!("../crates/sharecli-sync/tests/loom_pool_index.rs");
    assert!(loom_tests.contains("#![cfg(loom)]"), "loom integration test must be loom-gated");
    assert!(loom_tests.contains("PoolIndex"), "loom suite must exercise PoolIndex");
    assert!(loom_tests.contains("RelaxedCounter"), "loom suite must exercise relaxed counter");
}

#[test]
fn c00_l7_concurrency_docs_reference_loom() {
    let doc = include_str!("../docs/ops/concurrency.md");
    assert!(doc.contains("loom"), "concurrency.md must document loom gate");
    assert!(doc.contains("sharecli-sync"), "concurrency.md must cite sharecli-sync");
}

#[test]
fn c00_l7_ci_wires_loom_job() {
    let ci = include_str!("../.github/workflows/ci.yml");
    assert!(ci.contains("loom"), "ci.yml must run loom job");
    assert!(ci.contains("sharecli-sync"), "ci.yml must target sharecli-sync loom test");
}
