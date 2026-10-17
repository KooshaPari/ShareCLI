//! FR-002 — per-table defaults under `#[serde(default)]`
//! FR: FR-002
//!
//! Phase 1 task 1.3 (lane 8 HIGH): `#[serde(default)]` on the container `Config`
//! only fills fields whose *table is absent entirely*. A table that is present
//! but partial is deserialized by the child type, and serde's implicit rule for
//! `Option<T>` fills a missing key with `None` rather than with that type's
//! `Default`. Because the table then differs from its own `Default`, the
//! documented defaults (`max_memory_mb` 4096, `max_processes` 100) are silently
//! dropped.
//!
//! The audit also records the shape of the defect as two-tier and arbitrary:
//! some tables carry a struct-level `#[serde(default)]` and some do not, so the
//! behaviour depends on which table the operator happened to edit.
//!
//! Policy under test: **every table owns its defaults.** A present-but-empty or
//! present-but-partial table must deserialize exactly to that type's
//! `Default`, never to `None` and never by failing to parse.

use serde::Serialize;
use sharecli::config::{Config, RuntimeConfig};

/// Every struct the audit listed as missing a struct-level `#[serde(default)]`,
/// written as a present-but-empty table so nothing can mask the fill.
const EMPTY_TABLES: &str = r#"
[runtime]
[pool]
[monitoring]
[port]
[paths]
[project_limits]
[spawn]

[defaults.node]
"#;

/// Compare a deserialized sub-config against its own `Default` through its
/// serialized form, so no type under test needs to derive `PartialEq`.
fn assert_matches_own_default<T: Serialize + Default>(name: &str, actual: &T) {
    let expected = toml::to_string(&T::default()).expect("serialize default");
    let got = toml::to_string(actual).expect("serialize actual");
    assert_eq!(
        got, expected,
        "`{name}` present-but-empty must fill from its own Default, not from a \
         partial/None fill"
    );
}

/// The plan's named acceptance for task 1.3: a partial `[runtime]` table still
/// yields the documented 4096 / 100.
#[test]
fn runtime_config_from_partial_toml_uses_defaults() {
    let rt: RuntimeConfig =
        toml::from_str("node_path = \"/usr/local/bin/node\"").expect("parse partial runtime");

    assert_eq!(rt.node_path.as_deref(), Some("/usr/local/bin/node"));
    assert_eq!(
        rt.max_memory_mb,
        Some(4096),
        "a partial [runtime] table must keep the documented memory default"
    );
    assert_eq!(
        rt.max_processes,
        Some(100),
        "a partial [runtime] table must keep the documented process-count default"
    );
}

/// The same defect through the top-level `Config` path that `Config::load`
/// actually uses, so the fix is not only visible on the direct constructor.
#[test]
fn partial_runtime_table_keeps_defaults_in_config() {
    let toml_str = r#"
        [runtime]
        node_path = "/usr/local/bin/node"
    "#;

    let cfg: Config = toml::from_str(toml_str).expect("parse partial TOML");

    assert_eq!(cfg.runtime.node_path.as_deref(), Some("/usr/local/bin/node"));
    assert_eq!(cfg.runtime.max_memory_mb, Some(4096));
    assert_eq!(cfg.runtime.max_processes, Some(100));
}

/// The audit's "two-tier and arbitrary" observation, pinned directly: each of
/// the eight tables that lacked a struct-level `#[serde(default)]` must fill
/// from its own `Default` when present but empty.
#[test]
fn every_present_but_empty_table_fills_from_its_own_default() {
    let cfg: Config = toml::from_str(EMPTY_TABLES).expect("parse present-but-empty tables");

    assert_matches_own_default("runtime", &cfg.runtime);
    assert_matches_own_default("pool", &cfg.pool);
    assert_matches_own_default("monitoring", &cfg.monitoring);
    assert_matches_own_default("port", &cfg.port);
    assert_matches_own_default("paths", &cfg.paths);
    assert_matches_own_default("project_limits", &cfg.project_limits);
    assert_matches_own_default("spawn", &cfg.spawn);
    assert_matches_own_default("defaults.node", &cfg.defaults["node"]);
}

/// An entirely absent table must still come out as the default — the behaviour
/// `Config`'s own container-level `#[serde(default)]` already provides, kept as
/// a regression guard so the per-table fix cannot disturb it.
#[test]
fn absent_tables_still_come_from_config_default() {
    let cfg: Config = toml::from_str("").expect("parse empty TOML");
    let default = Config::default();

    assert_matches_own_default("runtime", &cfg.runtime);
    assert_eq!(cfg.runtime.max_memory_mb, default.runtime.max_memory_mb);
    assert_eq!(cfg.runtime.max_processes, default.runtime.max_processes);
}

/// End-to-end receipt: a partial `[runtime]` table on disk, loaded through the
/// production `Config::load()` path (which is what `sharecli serve` calls), still
/// yields the documented defaults. This is the TOML-driven path the audit flagged
/// as never asserted — the direct-constructor path was covered, the one that
/// actually runs was not.
#[test]
#[serial_test::serial]
fn load_from_disk_of_a_partial_runtime_table_keeps_defaults() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, "[runtime]\nnode_path = \"/usr/local/bin/node\"\n")
        .expect("write partial config");

    // SAFETY: serial test, scoped to this process for the duration of the call.
    unsafe { std::env::set_var("SHARECLI_CONFIG_PATH", &path) };
    let loaded = Config::load();
    unsafe { std::env::remove_var("SHARECLI_CONFIG_PATH") };

    let cfg = loaded.expect("load partial config from disk");
    assert_eq!(cfg.runtime.node_path.as_deref(), Some("/usr/local/bin/node"));
    assert_eq!(
        cfg.runtime.max_memory_mb,
        Some(4096),
        "the real load path must keep the documented memory default"
    );
    assert_eq!(
        cfg.runtime.max_processes,
        Some(100),
        "the real load path must keep the documented process-count default"
    );
}
