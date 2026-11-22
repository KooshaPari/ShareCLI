// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C02 L25 — serve HTTP rate limit wiring (FR-003).
//!
//! FR: FR-003

use sharecli::config::ServeConfig;
use sharecli::error_envelope::ErrorEnvelope;
use sharecli::serve_rate_limit::{is_probe_path, ServeRateLimit};

#[test]
fn fr003_rate_limit_envelope_golden_bytes() {
    let body = ErrorEnvelope::rate_limited("HTTP rate limit exceeded; retry later");
    let bytes = serde_json::to_vec(&body).expect("serialize envelope");
    assert_eq!(
        bytes,
        br#"{"error":{"type":"rate_limit_error","code":"rate_limited","message":"HTTP rate limit exceeded; retry later","request_id":null}}"#
    );
}

#[test]
fn fr003_probe_paths_exempt_from_rate_limit() {
    assert!(is_probe_path("/healthz"));
    assert!(is_probe_path("/readyz"));
    assert!(!is_probe_path("/metrics/prometheus"));
}

#[test]
fn fr003_serve_rate_limit_blocks_after_max() {
    let mut lim = ServeRateLimit::new(2, std::time::Duration::from_secs(60));
    assert!(lim.try_acquire());
    assert!(lim.try_acquire());
    assert!(!lim.try_acquire());
    assert!(lim.retry_after_secs() >= 1);
}

#[tokio::test]
async fn fr003_serve_middleware_wired_in_router() {
    // Drive the production router (`serve::build_router`, the same builder
    // `serve::run` serves) through `tower::ServiceExt::oneshot` so this covers
    // route table + `serve_rate_limit_middleware`, not a handler in isolation.
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sharecli::commands::serve::router_with_rate_limit;
    use sharecli::metrics::MetricsRegistry;
    use tokio::sync::watch;
    use tower::ServiceExt;

    // The exact config -> limiter resolution the server performs at startup.
    let cfg = ServeConfig {
        rate_limit_max: Some(2),
        rate_limit_window_secs: Some(60),
        ..ServeConfig::default()
    };
    // `from_env_or_config` honours `SHARECLI_SERVE_RATE_LIMIT_MAX/WINDOW_SECS`,
    // so build the router from an explicit limiter to keep the assertion
    // deterministic; the config path is covered by
    // `fr003_config_rate_limit_fields_present`.
    let resolved = ServeRateLimit::from_env_or_config(&cfg);
    if std::env::var_os("SHARECLI_SERVE_RATE_LIMIT_MAX").is_none() {
        let resolved = resolved.as_ref().expect("configured max must enable the limiter");
        assert_eq!(resolved.max_per_window(), 2);
        assert_eq!(resolved.window(), std::time::Duration::from_secs(60));
    } else {
        // Env overrides must still produce a usable limiter.
        assert!(resolved.is_some(), "env-supplied limit must enable the limiter");
    }

    let (shutdown_tx, _shutdown_rx) = watch::channel(false);
    let router = router_with_rate_limit(
        Arc::new(shutdown_tx),
        Some(ServeRateLimit::new(2, std::time::Duration::from_secs(60))),
        Arc::new(MetricsRegistry::new()),
    );

    let get =
        |uri: &'static str| Request::builder().uri(uri).body(Body::empty()).expect("build request");

    // Probes stay unlimited: the middleware short-circuits before `try_acquire`.
    for attempt in 0..5 {
        let response = router.clone().oneshot(get("/healthz")).await.expect("healthz request");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "probe /healthz must never be rate limited (attempt {attempt})"
        );
    }

    // A non-probe route goes through the limiter, which allows exactly `max`.
    for attempt in 0..2 {
        let response = router.clone().oneshot(get("/metrics/prometheus")).await.expect("request");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "request {attempt} is within the window and must pass"
        );
    }

    let response =
        router.clone().oneshot(get("/metrics/prometheus")).await.expect("over-limit request");
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "third request in the window must be rate limited by the wired middleware"
    );
    let retry_after = response
        .headers()
        .get("retry-after")
        .expect("429 must carry Retry-After")
        .to_str()
        .expect("Retry-After is ASCII");
    assert!(
        retry_after.parse::<u64>().map(|s| s >= 1).unwrap_or(false),
        "Retry-After must be a positive number of seconds, got {retry_after:?}"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let envelope: serde_json::Value = serde_json::from_slice(&body).expect("JSON envelope");
    assert_eq!(
        envelope["error"]["code"], "rate_limited",
        "429 must use the rate_limited error envelope"
    );

    // Probes remain reachable even after the limiter is exhausted.
    let response = router.clone().oneshot(get("/readyz")).await.expect("readyz request");
    assert_eq!(response.status(), StatusCode::OK, "/readyz must be exempt from the rate limit");
}

#[test]
fn fr003_config_rate_limit_fields_present() {
    let cfg = ServeConfig {
        rate_limit_max: Some(120),
        rate_limit_window_secs: Some(60),
        ..ServeConfig::default()
    };
    let lim = ServeRateLimit::from_env_or_config(&cfg).expect("limiter from config");
    assert_eq!(lim.max_per_window(), 120);
    assert_eq!(lim.window(), std::time::Duration::from_secs(60));
}
