// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L8 / FR-003 — jemalloc + dhat-heap feature gates for serve allocator policy.
//!
//! Evidence: `src/alloc.rs`, `docs/ops/memory.md`, `docs/ops/alloc-profiling.md`.

#[test]
fn c00_l8_jemalloc_feature_declared() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("jemalloc = [\"dep:tikv-jemallocator\"]"),
        "jemalloc feature must wire tikv-jemallocator"
    );
    assert!(manifest.contains("tikv-jemallocator"), "tikv-jemallocator optional dep must exist");
}

#[test]
fn c00_l8_dhat_heap_feature_declared() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("dhat-heap = [\"dep:dhat\"]"),
        "dhat-heap feature must wire dhat dev profiling"
    );
}

#[test]
fn c00_l8_alloc_module_global_allocator_present() {
    let src = include_str!("../src/alloc.rs");
    assert!(src.contains("#[global_allocator]"), "alloc.rs must declare global_allocator");
    assert!(src.contains("active_allocator_label"), "alloc.rs must expose allocator label helper");
}

#[test]
fn c00_l8_memory_docs_reference_features() {
    let memory = include_str!("../docs/ops/memory.md");
    assert!(memory.contains("jemalloc"), "memory.md must document jemalloc feature");
    assert!(memory.contains("dhat"), "memory.md must document dhat profiling");
}

#[test]
fn c00_l8_dhat_soft_avoids_clap_help_exit() {
    // FR-003 — clap --help uses process::exit and skips dhat Profiler Drop.
    let script = include_str!("../scripts/ops/dhat_soft.sh");
    assert!(
        !script.contains("\"$BIN\" --help"),
        "dhat_soft must not invoke --help (skips Drop / no dhat-heap.json)"
    );
    assert!(script.contains("completions bash"), "dhat_soft must use a normal-returning CLI path");
    assert!(
        script.contains("SHARECLI_DHAT_PROFILE=1"),
        "dhat_soft must explicitly opt into heap profiling"
    );
}
