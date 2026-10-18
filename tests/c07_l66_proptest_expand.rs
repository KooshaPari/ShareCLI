// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C07 L66 / FR-003 — property-based testing expand (config + cast registry + replay).
//!
//! Evidence: `docs/ops/config-proptest.md`, `proptest-regressions/`, `src/proptest_util.rs`.

#[test]
fn c07_l66_proptest_util_failure_persistence() {
    let src = include_str!("../src/proptest_util.rs");
    assert!(
        src.contains("FileFailurePersistence::SourceParallel"),
        "proptest_util must enable file-backed replay"
    );
    assert!(src.contains("proptest-regressions"), "proptest_util must cite regressions dir");
}

#[test]
fn c07_l66_config_validator_boundary_props() {
    let src = include_str!("../src/config_validator.rs");
    assert!(src.contains("prop_health_check_interval_boundary"));
    assert!(src.contains("prop_health_check_interval_out_of_range_fails"));
    assert!(src.contains("prop_pool_idle_timeout_boundary"));
    assert!(src.contains("prop_spawn_policy_concurrent_builds_valid"));
}

#[test]
fn c07_l66_cast_registry_proptest() {
    let src = include_str!("../src/cast/registry.rs");
    assert!(src.contains("prop_register_list_roundtrip"));
    assert!(src.contains("prop_pane_map_toml_roundtrip"));
}

#[test]
fn c07_l66_cast_address_roundtrip_proptest() {
    let src = include_str!("../src/cast/address.rs");
    assert!(src.contains("prop_local_address_display_parse_roundtrip"));
    assert!(src.contains("prop_peel_pane_window_roundtrip_indices"));
}

#[test]
fn c07_l66_regression_seeds_committed() {
    let seed = include_str!("../proptest-regressions/config_validator.txt");
    assert!(
        seed.contains("max_processes = 1"),
        "committed replay seed must cover boundary max_processes=1"
    );
}

#[test]
fn c07_l66_config_proptest_doc_live() {
    let doc = include_str!("../docs/ops/config-proptest.md");
    assert!(doc.contains("boundary"), "config-proptest.md must document boundary props");
    assert!(doc.contains("replay"), "config-proptest.md must document replay");
}
