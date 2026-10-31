// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Axum middleware for the `serve` HTTP listener.
//!
//! Three middlewares are wired onto the router (outermost first):
//!   1. `http_observability_middleware` -- W3C `traceparent` extract/inject
//!      and HTTP RED metrics. Runs on every request.
//!   2. `serve_rate_limit_middleware` -- sliding-window rate limit. Skips
//!      `/healthz` and `/readyz` so probes are never throttled.
//!   3. `serve_auth::require_bearer` -- bearer-token / JWT check (in
//!      `crate::serve_auth`). Probes remain public per the rate-limit
//!      exclusion in `serve_auth`.

use std::time::Instant;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tracing::Instrument;

use crate::error_envelope::ErrorEnvelope;
use crate::serve_rate_limit::is_probe_path;

use super::state::AppState;

/// Observability middleware: W3C `traceparent` extract/inject + HTTP RED metrics.
pub async fn http_observability_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().as_str().to_owned();
    let path = req.uri().path().to_owned();
    let incoming =
        req.headers().get("traceparent").and_then(|v| v.to_str().ok()).map(str::to_owned);
    if incoming.is_some() {
        state.metrics.counter("sharecli_tracecontext_extracted_total").inc();
    }

    let span = tracing::info_span!(
        "http.request",
        http.method = %method,
        http.route = %path,
        otel.kind = "server",
        traceparent = incoming.as_deref().unwrap_or(""),
    );

    let mut response = next.run(req).instrument(span).await;
    let status = response.status().as_u16();
    state.http_red.record(status, start.elapsed());

    let outgoing = incoming.unwrap_or_else(synthesize_traceparent);
    state.metrics.counter("sharecli_tracecontext_injected_total").inc();
    if let Ok(val) = HeaderValue::from_str(&outgoing) {
        response.headers_mut().insert(HeaderName::from_static("traceparent"), val);
    }
    response
}

/// Sliding-window HTTP rate limit (C02 L25). Probes `/healthz` and `/readyz` stay unlimited.
pub async fn serve_rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if is_probe_path(path) {
        return next.run(req).await;
    }

    let denied = {
        let mut guard = state.rate_limit.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(lim) => !lim.try_acquire(),
            None => false,
        }
    };

    if denied {
        let retry_after = {
            let guard = state.rate_limit.lock().unwrap_or_else(|e| e.into_inner());
            guard.as_ref().map(|l| l.retry_after_secs()).unwrap_or(1)
        };
        let mut response = ErrorEnvelope::rate_limited("HTTP rate limit exceeded; retry later")
            .into_response(StatusCode::TOO_MANY_REQUESTS);
        if let Ok(val) = HeaderValue::from_str(&retry_after.to_string()) {
            response.headers_mut().insert(HeaderName::from_static("retry-after"), val);
        }
        return response;
    }

    next.run(req).await
}

/// Best-effort W3C traceparent when the client did not send one.
pub fn synthesize_traceparent() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let pid = u128::from(std::process::id());
    let trace_id = format!("{:032x}", nanos ^ (pid << 64));
    let span_id = format!("{:016x}", (nanos as u64) ^ (pid as u64));
    format!("00-{trace_id}-{span_id}-01")
}
