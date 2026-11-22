// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli serve` -- lock-guarded HTTP + WebSocket dashboard server.
//!
//! GET  /healthz  -- liveness probe (JSON)
//! GET  /readyz   -- readiness probe (JSON; 503 if shutdown requested)
//! WS   /ws       -- streams periodic ProcessSummary snapshots as JSON,
//!                   plus thermal pressure events when pressure changes.

mod dashboard;
mod handlers;
mod middleware;
mod poller;
mod pressure;
mod prometheus;
mod run;
mod state;
mod ws;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use dashboard::{
    build_dashboard_ws_snapshot, DashboardAgentSummary, DashboardProcessRow, DashboardWsSnapshot,
};
pub use handlers::healthz_json;
pub use handlers::readyz_response;
pub use pressure::{parse_pressure_level, ThermalEvent};
pub use prometheus::{escape_label_value, render_prometheus_metrics};
pub use run::run;

/// Build the **production** serve router (single source of truth for the route
/// table and middleware stack used by `serve::run`).
///
/// Kept separate from `run` so tests can drive the exact routed surface without
/// binding a socket.
pub(crate) fn build_router(
    state: state::AppState,
    auth: crate::serve_auth::ServeAuth,
) -> axum::Router {
    use axum::middleware as axum_mw;
    use axum::routing::get;
    use axum::Router;

    use self::middleware::{http_observability_middleware, serve_rate_limit_middleware};

    Router::new()
        .route("/", get(handlers::dashboard))
        .route("/assets/dashboard/ui/{*path}", get(crate::dashboard_assets::serve))
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .route("/config", get(handlers::config_handler))
        .route("/health/processes", get(handlers::health_processes_handler))
        .route("/metrics/prometheus", get(handlers::metrics_prometheus_handler))
        .route("/debug/pprof/profile", get(crate::pprof_http::profile_handler))
        .route("/ws", get(ws::ws_handler))
        .layer(axum_mw::from_fn_with_state(state.clone(), http_observability_middleware))
        .layer(axum_mw::from_fn_with_state(state.clone(), serve_rate_limit_middleware))
        .layer(axum_mw::from_fn_with_state(auth, crate::serve_auth::require_bearer))
        .with_state(state)
}

/// Build a router over the real production route table (see [`build_router`])
/// that exposes `/healthz` and `/readyz`, sharing `shutdown_tx` as the exact
/// readiness state the `/readyz` route reads. Used by integration tests so they
/// exercise the routed surface rather than calling handlers directly.
pub fn health_ready_router(
    shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
) -> axum::Router {
    use std::sync::Arc;

    use tokio::sync::{broadcast, RwLock};

    let (thermal_tx, _) = broadcast::channel(1);
    let state = state::AppState {
        thermal_tx: Arc::new(thermal_tx),
        shutdown_tx,
        config: Arc::new(RwLock::new(crate::config::Config::default())),
        health_store: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        http_red: Arc::new(crate::http_red::HttpRedMetrics::default()),
        rate_limit: Arc::new(std::sync::Mutex::new(None)),
        metrics: Arc::new(crate::metrics::MetricsRegistry::new()),
    };
    build_router(state, crate::serve_auth::ServeAuth::open())
}
