// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//!
//! C01 climb-2 — sharecli lib serve_auth / spawn_policy / paths / health.
//! Boot trigger marker.

use sharecli::config::{PathsConfig, ServeConfig, SpawnPolicyConfig};
use sharecli::monitoring::HealthStatus;
use sharecli::serve_auth::ServeAuth;
use sharecli::spawn_policy::{is_build_harness, SpawnPolicy};

/// FR-003 / C01 — ServeAuth open/bearer modes + labels.
#[test]
fn fr003_serve_auth_open_and_bearer_modes() {
    std::env::remove_var("SHARECLI_SERVE_TOKEN");
    std::env::remove_var("SHARECLI_SERVE_AUTH_MODE");

    let open_cfg = ServeConfig {
        bearer_token: None,
        auth_mode: Some("open".into()),
        jwt: None,
        rate_limit_max: None,
        rate_limit_window_secs: None,
    };
    let open = ServeAuth::from_env_or_config(&open_cfg).expect("open");
    assert!(!open.enabled());
    assert_eq!(open.mode_label(), "open");
    assert!(open.check_bearer(None));

    let bearer_cfg = ServeConfig {
        bearer_token: Some("climb2-secret".into()),
        auth_mode: Some("bearer".into()),
        jwt: None,
        rate_limit_max: Some(10),
        rate_limit_window_secs: Some(5),
    };
    let auth = ServeAuth::from_env_or_config(&bearer_cfg).expect("bearer");
    assert!(auth.enabled());
    assert_eq!(auth.mode_label(), "bearer");
    assert!(!auth.check_bearer(None));
    assert!(auth.check_bearer(Some("Bearer climb2-secret")));
    assert!(auth.check_authorization(Some("Bearer climb2-secret")).is_ok());
    assert!(auth.check_authorization(None).is_err());
}

/// FR-003 / C01 — spawn_policy harness detection + permit acquire.
#[test]
fn fr003_spawn_policy_harness_and_permits() {
    assert!(is_build_harness("cargo"));
    assert!(is_build_harness("cmake"));
    assert!(!is_build_harness("node"));

    let cfg = SpawnPolicyConfig { nice_level: 5, max_concurrent_builds: 2, use_sccache: false };
    let policy = SpawnPolicy::new(cfg);
    assert_eq!(policy.available_permits(), 2);
    let permit = policy.try_acquire_build_permit().expect("permit");
    assert_eq!(policy.available_permits(), 1);
    drop(permit);
    assert_eq!(policy.available_permits(), 2);
    let env = policy.build_env_overrides();
    let _ = env.len();
}

/// FR-003 / C01 — PathsConfig defaults + HealthStatus transitions.
#[test]
fn fr003_paths_config_and_health_status() {
    let paths = PathsConfig::default();
    assert!(paths.discovery_path.contains("Phenotype") || !paths.discovery_path.is_empty());
    assert_eq!(paths.default_compose_output, "process-compose.yml");

    let mut health = HealthStatus::default();
    assert!(health.healthy);
    health.mark_unhealthy("climb2");
    assert!(!health.healthy);
    health.mark_healthy();
    assert!(health.healthy);
}
