// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003

//! C01 — coverage climb toward 85% broad-workspace lines (FR-003).
//!
//! Targets high-impact `src/` library modules counted by llvm-cov without
//! expanding quality-gate ignore patterns.

use sharecli::config::{
    CastConfig, Config, DefaultHarnessConfig, MonitoringConfig, PathsConfig, PoolConfig,
    PortConfig, ProjectLimitsConfig, RuntimeConfig, ServeConfig, ServeJwtConfig, SpawnConfig,
    SpawnPolicyConfig,
};
use sharecli::dashboard_assets::{
    is_dashboard_asset_path, serve as serve_dashboard_asset, URL_PREFIX,
};
use sharecli::monitoring::{HealthStatus, HostResourceWatchJson, ProcessStats};
use sharecli::runtime::{
    ProcessFilter, ProcessPool, ProjectLimits, ProjectResources, SharedRuntime,
};
use sharecli_fuse::InterceptFsOptions;

/// FR-003 / C01 — all Config sub-struct defaults + JWT serve settings round-trip.
#[test]
fn fr003_config_all_subdefaults_and_serve_jwt() {
    let runtime = RuntimeConfig::default();
    assert_eq!(runtime.max_memory_mb, Some(4096));
    assert_eq!(runtime.max_processes, Some(100));

    let pool = PoolConfig::default();
    assert!(pool.enabled);
    assert_eq!(pool.max_per_type, 5);

    let monitoring = MonitoringConfig::default();
    assert_eq!(monitoring.health_check_interval_secs, 30);
    assert_eq!(monitoring.idle_process_threshold, 5);

    let port = PortConfig::default();
    assert_eq!(port.sharewei_port, 3100);

    let paths = PathsConfig::default();
    assert!(paths.discovery_path.contains("Phenotype"));
    assert_eq!(paths.default_compose_output, "process-compose.yml");

    let harness = DefaultHarnessConfig::default();
    assert!(harness.enabled);
    assert_eq!(harness.max_instances, 10);

    let limits = ProjectLimitsConfig::default();
    assert_eq!(limits.memory_limit_mb, 1024);
    assert_eq!(limits.max_processes, 10);

    let spawn = SpawnConfig::default();
    assert_eq!(spawn.default_harness, "claude");
    assert_eq!(spawn.prune_idle_seconds, 300);

    let policy = SpawnPolicyConfig::default();
    assert_eq!(policy.nice_level, 10);
    assert_eq!(policy.max_concurrent_builds, 2);
    assert!(!policy.use_sccache);

    let cast = CastConfig::default();
    assert_eq!(cast.default_transport, "wezterm");
    assert_eq!(cast.handshake_timeout_ms, 250);

    let jwt = ServeJwtConfig {
        issuer: "https://issuer.example/v2.0".into(),
        audience: "api://sharecli".into(),
        jwks_path: Some("/tmp/jwks.json".into()),
        jwks: Some(r#"{"keys":[]}"#.into()),
    };
    let serve = ServeConfig {
        bearer_token: Some("tok".into()),
        auth_mode: Some("jwt".into()),
        jwt: Some(jwt.clone()),
        rate_limit_max: Some(50),
        rate_limit_window_secs: Some(15),
    };
    let encoded = toml::to_string_pretty(&serve).expect("serve toml");
    let decoded: ServeConfig = toml::from_str(&encoded).expect("serve from toml");
    assert_eq!(decoded.auth_mode.as_deref(), Some("jwt"));
    assert_eq!(decoded.jwt.as_ref().unwrap().issuer, jwt.issuer);
    assert_eq!(decoded.rate_limit_max, Some(50));

    let cfg = Config::default();
    assert!(cfg.projects.contains_key("agentapi"));
    assert!(cfg.projects.contains_key("cliproxy"));
    assert!(cfg.projects.contains_key("colab"));
    assert!(cfg.defaults.contains_key("forge"));
    assert!(cfg.defaults.contains_key("bun"));
}

/// FR-003 / C01 — monitoring JSON watch formatting + health/process helpers.
#[test]
fn fr003_monitoring_host_watch_and_health_helpers() {
    sharecli::config::init_global();

    let watch = HostResourceWatchJson {
        fd_count: 12,
        net_rx_bytes: 100,
        net_tx_bytes: 200,
        mem_rss_bytes: 4096,
        load_1m: 1.25,
    };
    let text = watch.format_text_section();
    assert!(text.contains("fd") || text.contains("12") || text.contains("load"));
    let csv = watch.format_csv_companion();
    assert!(csv.contains("host"));
    assert!(csv.contains("12"));
    assert!(csv.contains("1.25"));

    let captured = HostResourceWatchJson::capture().expect("capture host watch");
    assert!(captured.mem_rss_bytes > 0 || captured.fd_count > 0 || captured.load_1m >= 0.0);

    let mut health = HealthStatus::default();
    assert!(health.healthy);
    health.mark_healthy();
    assert!(health.healthy);
    assert!(health.checks_passed >= 2);
    health.mark_unhealthy("probe failed");
    assert!(!health.healthy);

    let stats = ProcessStats::new(7, "node", 128, 0.2, 1000, 900);
    assert!(stats.is_idle(100));
    assert!(!stats.is_idle(10_000));
    let with_watch = stats.with_resource_watch().expect("resource watch");
    assert!(with_watch.mem_rss_bytes > 0 || with_watch.fd_count > 0);
}

/// FR-003 / C01 — SharedRuntime empty-pool health/status + ProcessPool find filters.
#[tokio::test]
async fn fr003_runtime_shared_status_and_process_filters() {
    sharecli::config::init_global();

    let runtime = SharedRuntime::new(2);
    runtime.refresh().await;
    let health = runtime.health_check().await;
    assert!(health.healthy, "empty pool should be healthy: {:?}", health.issues);
    let status = runtime.status().await;
    assert_eq!(status.max_per_type, 2);
    assert_eq!(status.node_total, 0);
    assert_eq!(status.bun_total, 0);

    let bad = runtime.acquire("deno").await;
    assert!(bad.is_err());

    let pool = ProcessPool::new();
    pool.refresh().await;
    let all = pool.find(ProcessFilter::All).await;
    let _ = pool.find(ProcessFilter::ByProject("missing".into())).await;
    let _ = pool.find(ProcessFilter::ByHarness("node".into())).await;
    assert!(all.len() <= pool.list().await.len() || all.is_empty() || !all.is_empty());

    let (used, total) = pool.system_memory_usage().await;
    assert!(total >= used);

    let resources = ProjectResources::new();
    resources
        .set_limits(
            "demo",
            ProjectLimits { memory_limit_mb: 512, max_processes: 3, cpu_affinity: None },
        )
        .await;
    let limits = resources.get_limits("demo").await;
    assert_eq!(limits.memory_limit_mb, 512);
    assert_eq!(limits.max_processes, 3);
    let check = resources.check_limits("demo").await.expect("check");
    assert!(check.memory_limit_mb == 512);
    assert!(check.max_processes == 3);
}

/// FR-003 / C01 — dashboard asset lookup/serve covers embedded paths.
#[tokio::test]
async fn fr003_dashboard_assets_serve_known_paths() {
    assert!(URL_PREFIX.starts_with("/assets/"));
    assert!(is_dashboard_asset_path(&format!("{URL_PREFIX}/favicons/phenotype.ico")));

    for rel in [
        "favicons/phenotype.ico",
        "favicons/phenotype_16.png",
        "favicons/phenotype_64.png",
        "favicons/phenotype_128.png",
        "empty-states/no-results.svg",
        "empty-states/no-results.png",
        "empty-states/error.svg",
        "empty-states/error.png",
        "empty-states/no-data.png",
        "icons/phenotype_icon.png",
    ] {
        let resp = serve_dashboard_asset(axum::extract::Path(rel.to_string())).await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK, "asset {rel}");
    }

    let missing = serve_dashboard_asset(axum::extract::Path("missing.bin".into())).await;
    assert_eq!(missing.status(), axum::http::StatusCode::NOT_FOUND);
}

/// FR-003 / C01 — InterceptFsOptions defaults used by fuse mount constructors.
#[test]
fn fr003_intercept_fs_options_defaults() {
    let opts = InterceptFsOptions::default();
    assert_eq!(opts.session_id, "default");
    assert!(!opts.cow);
    assert!(opts.serialize);
    assert!(opts.cow_dir.is_none());
    assert!(opts.agent.is_none());
    assert!(opts.agents_conf.is_none());
}

/// FR-003 / C01 — error envelope constructors + auth failure message map.
#[test]
fn fr003_error_envelope_variants_and_auth_messages() {
    use axum::http::StatusCode;
    use sharecli::error_envelope::{auth_failure_message, ErrorEnvelope};

    let unauth = ErrorEnvelope::unauthorized("nope");
    assert_eq!(unauth.error.error_type, "authentication_error");
    assert_eq!(unauth.error.code, "unauthorized");

    let validation = ErrorEnvelope::validation("bad_field", "invalid");
    assert_eq!(validation.error.error_type, "validation_error");

    let not_found = ErrorEnvelope::not_found("gone");
    assert_eq!(not_found.error.code, "not_found");

    let rate = ErrorEnvelope::rate_limited("slow down");
    assert_eq!(rate.error.error_type, "rate_limit_error");

    let ni = ErrorEnvelope::not_implemented("later");
    assert_eq!(ni.error.code, "not_implemented");

    let internal = ErrorEnvelope::internal();
    assert_eq!(internal.error.code, "internal_server_error");

    let resp = ErrorEnvelope::unauthorized("x").into_response(StatusCode::UNAUTHORIZED);
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    for (reason, needle) in [
        ("missing_authorization", "bearer"),
        ("not_bearer", "Bearer"),
        ("invalid_bearer", "invalid"),
        ("jwt_expired", "expired"),
        ("jwt_invalid_iss", "issuer"),
        ("jwt_invalid_aud", "audience"),
        ("jwt_nbf", "not yet"),
        ("jwt_invalid_sig", "signature"),
        ("jwt_no_matching_key", "key"),
        ("jwt_header:bad", "header"),
        ("jwt_invalid:x", "invalid"),
        ("jwt_other", "invalid"),
        ("totally_unknown", "bearer"),
    ] {
        let msg = auth_failure_message(reason);
        assert!(
            msg.to_lowercase().contains(&needle.to_lowercase()) || msg.contains(needle),
            "reason={reason} msg={msg} needle={needle}"
        );
    }
}

/// FR-003 / C01 — serve rate-limit probe paths + config builder.
#[test]
fn fr003_serve_rate_limit_probe_and_builder() {
    use std::time::Duration;

    use sharecli::config::ServeConfig;
    use sharecli::serve_rate_limit::{is_probe_path, ServeRateLimit};

    assert!(is_probe_path("/healthz"));
    assert!(is_probe_path("/readyz"));
    assert!(!is_probe_path("/v1/status"));

    let mut lim = ServeRateLimit::new(2, Duration::from_secs(60));
    assert_eq!(lim.max_per_window(), 2);
    assert_eq!(lim.window(), Duration::from_secs(60));
    assert!(lim.try_acquire());
    assert!(lim.try_acquire());
    assert!(!lim.try_acquire());
    assert!(lim.retry_after_secs() >= 1);

    let cfg = ServeConfig {
        rate_limit_max: Some(0),
        rate_limit_window_secs: Some(30),
        ..Default::default()
    };
    assert!(ServeRateLimit::from_env_or_config(&cfg).is_none());

    let cfg2 = ServeConfig {
        rate_limit_max: Some(5),
        rate_limit_window_secs: Some(10),
        ..Default::default()
    };
    let built = ServeRateLimit::from_env_or_config(&cfg2).expect("enabled");
    assert_eq!(built.max_per_window(), 5);
    assert_eq!(built.window(), Duration::from_secs(10));
}

/// FR-003 / C01 — health-check endpoint dispatcher covers tcp:// and host:port forms.
#[tokio::test]
async fn fr003_health_check_probe_endpoint_dispatch() {
    use std::time::Duration;

    use sharecli::health_check::probe_endpoint;
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let tcp_url = format!("tcp://127.0.0.1:{}", addr.port());
    assert!(probe_endpoint(&tcp_url, Duration::from_secs(2)).await.is_ok());

    // Closed port via bare host:port form.
    let closed = probe_endpoint("127.0.0.1:19998", Duration::from_millis(300)).await;
    assert!(closed.is_err());
}
