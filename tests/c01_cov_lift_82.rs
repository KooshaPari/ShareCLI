// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//! T-810 C01 coverage lift prep toward 82% — uncovered helpers (config, registry, fleet).

use sharecli::cast::address::PaneAddress;
use sharecli::cast::registry::PaneRegistry;
use sharecli::config::{CastConfig, Config};
use sharecli_fleet::{FleetRegistry, ThermalGovernor, ThermalLevel};

/// FR-003 — Config defaults embed projects/harness presets and round-trip via TOML.
#[test]
fn fr003_config_default_projects_and_cast_roundtrip() {
    let cfg = Config::default();
    assert!(cfg.projects.contains_key("helios-cli"));
    assert!(cfg.projects.contains_key("portage"));
    assert!(cfg.defaults.contains_key("claude"));
    // CastConfig defaults
    let cast = CastConfig::default();
    assert_eq!(cast.default_transport, "wezterm");
    assert_eq!(cast.handshake_timeout_ms, 250);
    assert_eq!(cast.max_retry_attempts, 3);
    // Round-trip custom config through TOML to exercise serde defaults
    let custom = Config {
        cast: CastConfig {
            default_transport: "ghostty".into(),
            pane_map_path: Some("/tmp/panes.toml".into()),
            handshake_timeout_ms: 500,
            max_retry_attempts: 5,
            retry_backoff_ms: 400,
        },
        ..Config::default()
    };
    let toml_text = toml::to_string_pretty(&custom).expect("serialize config");
    let loaded: Config = toml::from_str(&toml_text).expect("deserialize config");
    assert_eq!(loaded.cast.default_transport, "ghostty");
    assert_eq!(loaded.cast.handshake_timeout_ms, 500);
    assert_eq!(loaded.cast.max_retry_attempts, 5);
}

/// FR-003 — PaneRegistry helpers in isolated tmpdir (register / resolve / list / unregister).
#[test]
fn fr003_registry_helpers_tmpdir_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reg = PaneRegistry::new_in(tmp.path()).expect("registry new_in");
    assert!(reg.path().ends_with("pane-map.toml"));

    // Empty list
    let list = reg.list().expect("list empty");
    assert!(list.is_empty());

    // Register and resolve
    let addr = PaneAddress::parse("mbp:local:1:2").expect("parse address");
    reg.register("pane-a", &addr).expect("register pane-a");
    let resolved = reg.resolve("pane-a").expect("resolve").expect("some");
    assert_eq!(resolved, addr);

    // List contains one
    let list = reg.list().expect("list after register");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, "pane-a");

    // Second pane
    let addr2 = PaneAddress::parse("mbp:tailscale:0:1").expect("parse addr2");
    reg.register("pane-b", &addr2).expect("register pane-b");
    assert_eq!(reg.list().expect("list 2").len(), 2);

    // Unregister
    reg.unregister("pane-a").expect("unregister");
    assert!(reg.resolve("pane-a").expect("resolve after unregister").is_none());
    assert_eq!(reg.list().expect("list after unregister").len(), 1);

    // Validate name rejects forbidden chars via register
    let bad = reg.register("bad/name", &addr);
    assert!(bad.is_err(), "expected invalid name error");

    // Well-known paths helper exercised for coverage
    let paths = sharecli::paths::well_known_paths();
    assert!(!paths.config_dir.as_os_str().is_empty());
    assert!(!paths.state_dir.as_os_str().is_empty());
}

/// FR-003 — Fleet helpers: ThermalGovernor mock levels and FleetRegistry subject helper.
#[test]
fn fr003_fleet_thermal_and_registry_helpers() {
    // ThermalGovernor with_mock covers thermal.rs poll mock branch
    for level in [ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red] {
        let gov = ThermalGovernor::with_mock(level);
        assert_eq!(gov.poll().expect("poll mock"), level);
    }
    let gov_default = ThermalGovernor::new();
    // Default governor polls real system — should return Ok (Green on CI without thermal zone)
    assert!(gov_default.poll().is_ok());

    // FleetRegistry disconnected helpers (registry.rs uncovered paths)
    let reg = FleetRegistry::disconnected().with_subject_prefix("test.prefix");
    assert_eq!(reg.subject_for("dev-123"), "test.prefix.devices.dev-123");
    let reg2 = FleetRegistry::disconnected();
    assert_eq!(
        reg2.subject_for("abc"),
        format!("{}.devices.abc", sharecli_fleet::DEFAULT_SUBJECT_PREFIX)
    );
}
