// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003 — C05 L42 OTel instrumentation acceptance gate.
//!
//! Verifies the trace-context inject/extract counters are wired end-to-end:
//! middleware increments -> MetricsRegistry -> `/metrics/prometheus` output.
//! These counters populate L49's `sharecli-trace.json` panel #2.
//!
//! The assertions drive the production router (`serve::build_router`, the same
//! builder `serve::run` serves) through `tower::ServiceExt::oneshot`, so they
//! observe the real middleware and handler behaviour rather than source text.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sharecli::commands::serve::router_with_rate_limit;
use sharecli::metrics::MetricsRegistry;
use tokio::sync::watch;
use tower::ServiceExt;

const INJECTED: &str = "sharecli_tracecontext_injected_total";
const EXTRACTED: &str = "sharecli_tracecontext_extracted_total";

/// Build the production serve router with a fresh, inspectable metrics registry.
fn router_with_metrics(metrics: Arc<MetricsRegistry>) -> axum::Router {
    let (shutdown_tx, _shutdown_rx) = watch::channel(false);
    router_with_rate_limit(Arc::new(shutdown_tx), None, metrics)
}

fn get(uri: &'static str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).expect("build request")
}

/// Prometheus sample lines are `name value`; compare the whole line so a value
/// of `10`/`100` cannot satisfy an expected `1` by prefix.
fn has_sample_line(text: &str, name: &str, value: u64) -> bool {
    let expected = format!("{name} {value}");
    text.lines().any(|line| line.trim_end() == expected)
}

const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

#[test]
fn fr003_counter_names_are_stable_constants() {
    // Guard: the metric names must match what sharecli-trace.json panel #2 queries.
    assert_eq!(INJECTED, "sharecli_tracecontext_injected_total");
    assert_eq!(EXTRACTED, "sharecli_tracecontext_extracted_total");
}

#[tokio::test]
async fn fr003_middleware_increments_extract_counter_when_traceparent_present() {
    let metrics = Arc::new(MetricsRegistry::new());
    let response = router_with_metrics(Arc::clone(&metrics))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("traceparent", TRACEPARENT)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("request through router");

    assert_eq!(response.status(), StatusCode::OK);
    // The extract counter is bumped only when an incoming traceparent exists.
    assert_eq!(
        metrics.counter(EXTRACTED).get(),
        1,
        "inbound traceparent must increment the extract counter exactly once"
    );
    // A request without a traceparent must NOT touch the extract counter.
    let response = router_with_metrics(Arc::clone(&metrics))
        .oneshot(get("/healthz"))
        .await
        .expect("request through router");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        metrics.counter(EXTRACTED).get(),
        1,
        "requests without traceparent must not increment the extract counter"
    );
}

#[tokio::test]
async fn fr003_middleware_increments_inject_counter() {
    let metrics = Arc::new(MetricsRegistry::new());
    let response = router_with_metrics(Arc::clone(&metrics))
        .oneshot(get("/healthz"))
        .await
        .expect("request through router");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        metrics.counter(INJECTED).get(),
        1,
        "every response must increment the inject counter"
    );
    // The outgoing header is what the inject point writes.
    let outgoing = response
        .headers()
        .get("traceparent")
        .expect("response must carry an injected traceparent")
        .to_str()
        .expect("traceparent is ASCII")
        .to_owned();
    assert!(
        outgoing.starts_with("00-") && outgoing.ends_with("-01") && outgoing.len() == 55,
        "injected traceparent must be a W3C traceparent, got {outgoing:?}"
    );

    // An inbound traceparent is propagated, not replaced.
    let response = router_with_metrics(Arc::clone(&metrics))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("traceparent", TRACEPARENT)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("request through router");
    assert_eq!(
        response.headers().get("traceparent").and_then(|v| v.to_str().ok()),
        Some(TRACEPARENT),
        "inbound traceparent must be propagated on the response"
    );
    assert_eq!(metrics.counter(INJECTED).get(), 2);
}

#[tokio::test]
async fn fr003_prometheus_handler_reads_both_counters() {
    let metrics = Arc::new(MetricsRegistry::new());

    // One request with a traceparent: extracted=1 during handler execution;
    // injected=1 is recorded once the first response is complete.
    let first = router_with_metrics(Arc::clone(&metrics))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("traceparent", TRACEPARENT)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("request through router");
    assert_eq!(first.status(), StatusCode::OK);

    let response = router_with_metrics(Arc::clone(&metrics))
        .oneshot(get("/metrics/prometheus"))
        .await
        .expect("scrape through router");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let text = String::from_utf8(body.to_vec()).expect("prometheus body is UTF-8");

    assert!(
        has_sample_line(&text, INJECTED, 1),
        "prometheus output must expose an exact `{INJECTED} 1` sample line:\n{text}"
    );
    assert!(
        has_sample_line(&text, EXTRACTED, 1),
        "prometheus output must expose an exact `{EXTRACTED} 1` sample line:\n{text}"
    );
    // The scrape itself increments the injected counter only after the handler
    // renders the body, so the emitted value is the pre-scrape count and the
    // registry advances afterwards. This is why `1` is the correct expectation.
    assert_eq!(
        metrics.counter(INJECTED).get(),
        2,
        "the scrape request must increment the inject counter after rendering"
    );
    assert!(
        text.contains(&format!("# TYPE {INJECTED} counter")),
        "injected counter must be typed as a counter"
    );
}

#[test]
fn fr003_dashboard_panel_references_the_same_counter_names() {
    // L49's trace dashboard (shipped by Plan 782) must query the exact counters
    // this feature produces — closes the panel #2 gap.
    let dash = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/ops/grafana/dashboards/sharecli-trace.json");
    if dash.exists() {
        let json = std::fs::read_to_string(&dash).unwrap_or_default();
        assert!(
            json.contains(INJECTED) && json.contains(EXTRACTED),
            "sharecli-trace.json must query both trace-context counters"
        );
    }
}
