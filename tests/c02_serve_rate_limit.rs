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

#[test]
fn fr003_serve_middleware_wired_in_router() {
    let serve_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/serve.rs"),
    )
    .expect("read serve.rs");
    assert!(
        serve_rs.contains("serve_rate_limit_middleware"),
        "serve.rs must define rate limit middleware"
    );
    assert!(
        serve_rs.contains("ServeRateLimit::from_env_or_config"),
        "serve.rs must resolve rate limit from config/env"
    );
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
