//! C00 L1 / FR-003 — Phase 1 util facade boundary for `src/lib.rs`.
//!
//! Root `pub mod` must stay Tier A/B (+ `util` umbrella). Parity/expansion
//! modules live under `sharecli::util` (file paths unchanged).
//!
//! Plan: `docs/ops/lib-sprawl-plan.md` Phase 1.
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

/// Product / ops modules that may remain as root `pub mod` (Tier A+B + util).
const ROOT_PUB_MOD_ALLOWLIST: &[&str] = &[
    "agent_call_policy",
    "audit_log",
    "cast",
    "commands",
    "config",
    "config_validator",
    "config_loader",
    "config_merger",
    "config_watcher",
    "coordination",
    "dashboard_assets",
    "env_manager",
    "error",
    "error_envelope",
    "health",
    "health_check",
    "http_red",
    "log_sink",
    "metrics",
    "monitoring",
    "notifier",
    "otel",
    "paths",
    "pprof_http",
    "pool_index",
    "proc_table",
    "progress",
    "proptest_util",
    "pyroscope_stub",
    "runtime",
    "scheduler",
    "serve_auth",
    "serve_lock",
    "serve_rate_limit",
    "session",
    "signals",
    "spawn_policy",
    "tray_http",
    "util",
    "watchdog",
];

fn lib_rs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs")
}

fn root_pub_mods(src: &str) -> Vec<String> {
    let mut mods = Vec::new();
    let mut depth = 0usize;
    for line in src.lines() {
        let trimmed = line.trim();
        // Track brace depth so we ignore `pub mod` inside `pub mod util { ... }`.
        let opens = line.chars().filter(|&c| c == '{').count();
        let closes = line.chars().filter(|&c| c == '}').count();
        if depth == 0 && trimmed.starts_with("pub mod ") {
            let name = trimmed
                .trim_start_matches("pub mod ")
                .trim_end_matches(';')
                .trim_end_matches('{')
                .trim()
                .to_string();
            if !name.is_empty() {
                mods.push(name);
            }
        }
        depth = depth + opens - closes;
    }
    mods
}

#[test]
fn fr003_root_pub_mods_are_product_plus_util_only() {
    let src = fs::read_to_string(lib_rs()).expect("read src/lib.rs");
    let mods = root_pub_mods(&src);
    assert!(
        mods.iter().any(|m| m == "util"),
        "expected `pub mod util` umbrella (lib-sprawl Phase 1)"
    );
    let unexpected: Vec<_> =
        mods.iter().filter(|m| !ROOT_PUB_MOD_ALLOWLIST.contains(&m.as_str())).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "root pub mod sprawl still present: {unexpected:?} (count={})",
        unexpected.len()
    );
    assert!(
        mods.len() <= ROOT_PUB_MOD_ALLOWLIST.len(),
        "too many root pub mods: {} > {}",
        mods.len(),
        ROOT_PUB_MOD_ALLOWLIST.len()
    );
}

#[test]
fn fr003_util_facade_exposes_parity_sample() {
    // Compile-time + runtime smoke: Tier C reachable via util path.
    let _ = sharecli::util::toml_lite::parse("a = 1");
    let _ = sharecli::util::bloom::BloomFilter::new(64, 3);
    let _ = sharecli::util::levenshtein::distance("a", "b");
}

#[test]
fn fr003_legacy_root_parity_path_is_gone() {
    // Phase 1: Tier C is util-namespaced only (no root `pub mod toml_lite`).
    // Fuzz + callers use `sharecli::util::toml_lite` (see fuzz/fuzz_targets/toml_lite.rs).
    let src = fs::read_to_string(lib_rs()).expect("read src/lib.rs");
    assert!(
        !src.lines().any(|l| l.trim() == "pub mod toml_lite;"),
        "toml_lite must not be a root pub mod after Phase 1"
    );
}
