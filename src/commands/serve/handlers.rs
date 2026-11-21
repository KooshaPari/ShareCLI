//! Plain HTTP request handlers for the `serve` listener.
//!
//! WebSocket handling lives in `super::ws`; Prometheus rendering lives
//! in `super::prometheus`. The functions here are thin: each takes
//! `State<AppState>` (or none for the static dashboard), and delegates
//! to the appropriate renderer / store.

use std::collections::HashMap;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::response::{IntoResponse, Json};
use serde_json::json;
use tracing::instrument;

use super::prometheus::render_prometheus_metrics;
use super::state::AppState;
use crate::error_envelope::ErrorEnvelope;
use crate::http_red::render_http_red_metrics;
use crate::runtime::ProcessPool;

const DASHBOARD_HTML: &str = include_str!("../../dashboard.html");

#[instrument]
pub async fn dashboard() -> impl IntoResponse {
    let html = crate::tray_http::inject_dashboard_traceparent(DASHBOARD_HTML);
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html)
}

/// Pure liveness JSON body (unit-tested without spinning the HTTP server).
pub fn healthz_json() -> serde_json::Value {
    json!({"status": "ok"})
}

/// Pure readiness decision: `(status, body)` from the shutdown flag.
pub fn readyz_response(shutdown_requested: bool) -> (StatusCode, serde_json::Value) {
    if shutdown_requested {
        (StatusCode::SERVICE_UNAVAILABLE, json!({"status": "unavailable"}))
    } else {
        (StatusCode::OK, json!({"status": "ok"}))
    }
}

#[instrument]
pub async fn healthz() -> impl IntoResponse {
    Json(healthz_json())
}

/// `GET /readyz` — readiness probe; 200 while serving, 503 once shutdown is requested.
#[instrument(skip(state))]
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let (status, body) = readyz_response(*state.shutdown_tx.borrow());
    (status, Json(body))
}

/// `GET /config` — returns the current live config as JSON.
///
/// The value here reflects the last successful hot-reload; it updates
/// in-place whenever the config file is saved with valid TOML.
#[instrument(skip(state))]
pub async fn config_handler(State(state): State<AppState>) -> Response {
    let cfg = state.config.read().await.clone();
    match serde_json::to_value(cfg) {
        Ok(v) => Json(v).into_response(),
        Err(_) => ErrorEnvelope::internal().into_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /health/processes` — returns health status of all monitored processes.
#[instrument(skip(state))]
pub async fn health_processes_handler(State(state): State<AppState>) -> impl IntoResponse {
    let map = state.health_store.lock().await;
    // Serialize to a plain JSON map: process_name → status fields.
    let out: HashMap<&str, serde_json::Value> = map
        .iter()
        .map(|(name, status)| {
            (
                name.as_str(),
                json!({
                    "healthy": status.healthy,
                    "consecutive_failures": status.consecutive_failures,
                    "last_error": status.last_error,
                }),
            )
        })
        .collect();
    Json(serde_json::to_value(out).unwrap_or_else(|_| json!({})))
}

/// `GET /metrics/prometheus` — Prometheus text-format metrics for all tracked processes.
#[instrument(skip(state))]
pub async fn metrics_prometheus_handler(State(state): State<AppState>) -> impl IntoResponse {
    let pool = ProcessPool::new();
    let processes = pool.list().await;
    let health_map = state.health_store.lock().await;
    let mut body = render_prometheus_metrics(&processes, &health_map);
    render_http_red_metrics(&mut body, &state.http_red.snapshot());
    // C05 L42: trace-context counters consumed by the L49 sharecli-trace dashboard panel.
    let injected = state.metrics.counter("sharecli_tracecontext_injected_total").get();
    let extracted = state.metrics.counter("sharecli_tracecontext_extracted_total").get();
    body.push_str("# HELP sharecli_tracecontext_injected_total W3C traceparent headers injected by sharecli serve\n");
    body.push_str("# TYPE sharecli_tracecontext_injected_total counter\n");
    body.push_str(&format!("sharecli_tracecontext_injected_total {injected}\n"));
    body.push_str("# HELP sharecli_tracecontext_extracted_total W3C traceparent headers extracted from inbound requests\n");
    body.push_str("# TYPE sharecli_tracecontext_extracted_total counter\n");
    body.push_str(&format!("sharecli_tracecontext_extracted_total {extracted}\n"));
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body)
}
