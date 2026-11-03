// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Entry point for `sharecli serve`.
//!
//! Builds the axum router, acquires the serve lock, starts the thermal
//! poller and the config hot-reload watcher, and runs the HTTP listener
//! until the shutdown signal fires (SIGINT/SIGTERM or thermal Red).

use std::sync::Arc;

use anyhow::Result;
use axum::middleware;
use axum::routing::get;
use axum::Router;
use serde_json::json;
use sharecli_fleet::thermal::ThermalGovernor;
use tokio::sync::{broadcast, watch, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, warn};

use crate::audit_log;
use crate::config::Config;
use crate::config_watcher::ConfigWatcher;
use crate::health_check::{HealthCheckScheduler, HealthCheckStore};
use crate::http_red::HttpRedMetrics;
use crate::metrics::MetricsRegistry;
use crate::notifier::Notifier;
use crate::serve_auth::{self, ServeAuth};
use crate::serve_lock::{decide, probe, Decision, OnConflict, ServeLock, ServeState};
use crate::serve_rate_limit::ServeRateLimit;
use crate::shutdown::serve_shutdown_signal;

use super::handlers::{
    config_handler, dashboard, health_processes_handler, healthz, metrics_prometheus_handler,
    readyz,
};
use super::middleware::{http_observability_middleware, serve_rate_limit_middleware};
use super::poller::thermal_poll_task;
use super::pressure::ThermalEvent;
use super::state::AppState;
use super::ws::ws_handler;

/// Entry point for `sharecli serve`.
#[instrument(skip(on_conflict), fields(bind = %bind))]
pub async fn run(bind: &str, on_conflict: OnConflict) -> Result<()> {
    let state = probe("sharecli")?;

    match decide(&state, on_conflict) {
        Decision::Attach => {
            let url = match &state {
                ServeState::Running { info, .. } => info.url.clone(),
                ServeState::Free => unreachable!(),
            };
            info!(%url, "sharecli serve: attaching to existing instance");
            println!("sharecli serve already running at {url}");
            return Ok(());
        }
        Decision::Abort => {
            let url = match &state {
                ServeState::Running { info, .. } => info.url.clone(),
                ServeState::Free => unreachable!(),
            };
            anyhow::bail!("serve already running at {url}");
        }
        Decision::Serve | Decision::Replace => {}
    }

    let url = format!("http://{bind}");
    let lock = ServeLock::try_acquire("sharecli", url.clone())?.ok_or_else(|| {
        anyhow::anyhow!("could not acquire serve lock -- another instance is running")
    })?;

    info!(%url, %bind, "sharecli serve: starting HTTP listener");

    // Log current thermal level on startup.
    let gov = ThermalGovernor::new();
    match gov.poll() {
        Ok(level) => info!("sharecli serve: startup thermal pressure = {:?}", level),
        Err(e) => warn!("sharecli serve: could not read thermal pressure: {e}"),
    }

    let (thermal_tx, _) = broadcast::channel::<ThermalEvent>(64);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let cancel = CancellationToken::new();
    let thermal_cancel = cancel.child_token();
    let config_cancel = cancel.child_token();

    // Build the live config and start the hot-reload watcher.
    let initial_config = Config::load().unwrap_or_default();
    let config_arc = Arc::new(RwLock::new(initial_config.clone()));
    let (cfg_tx, mut cfg_rx) = watch::channel(initial_config.clone());

    let config_path = dirs::config_dir()
        .map(|d| d.join("sharecli").join("config.toml"))
        .unwrap_or_else(|| std::path::PathBuf::from("config.toml"));

    // `_config_watcher` is kept alive by the AppState so the file watch persists
    // for the lifetime of the server.
    let _config_watcher = ConfigWatcher::new(config_path, cfg_tx)
        .inspect_err(|e| {
            warn!("config_watcher: could not start file watcher: {e}; hot-reload disabled");
        })
        .ok();

    // Spawn a task that propagates config-reload signals into the shared RwLock.
    let config_arc_writer = Arc::clone(&config_arc);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = config_cancel.cancelled() => {
                    info!("serve: config reload task cancelled");
                    return;
                }
                changed = cfg_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    let new_cfg = cfg_rx.borrow().clone();
                    *config_arc_writer.write().await = new_cfg;
                    info!("serve: config hot-reloaded");
                }
            }
        }
    });

    // Build notifier from config.
    let notifier = Notifier::new(initial_config.notifications.clone());

    // Build the health-check store and start per-process schedulers.
    let health_store: HealthCheckStore =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    {
        let scheduler =
            HealthCheckScheduler::with_notifier(Arc::clone(&health_store), Arc::clone(&notifier));
        scheduler.start(initial_config.health_checks.clone());
    }

    let auth = ServeAuth::from_env_or_config(&initial_config.serve)
        .map_err(|e| anyhow::anyhow!("serve AuthN config error: {e}"))?;
    if auth.enabled() {
        info!(
            "sharecli serve: AuthN mode={} (probes /healthz /readyz remain public)",
            auth.mode_label()
        );
        audit_log::emit("auth_enabled", json!({ "mode": auth.mode_label() }));
    } else {
        info!("sharecli serve: AuthN disabled (set SHARECLI_SERVE_TOKEN or [serve.jwt])");
        audit_log::emit("auth_disabled", json!({ "mode": "open" }));
    }
    audit_log::emit("serve_start", json!({ "url": url, "bind": bind }));

    let rate_limit = ServeRateLimit::from_env_or_config(&initial_config.serve);
    if let Some(ref lim) = rate_limit {
        info!(
            max = lim.max_per_window(),
            window_secs = lim.window().as_secs(),
            "sharecli serve: HTTP rate limit enabled (probes /healthz /readyz exempt)"
        );
    }

    let state = AppState {
        thermal_tx: Arc::new(thermal_tx),
        shutdown_tx: Arc::new(shutdown_tx),
        config: config_arc,
        health_store,
        http_red: Arc::new(HttpRedMetrics::default()),
        rate_limit: Arc::new(std::sync::Mutex::new(rate_limit)),
        metrics: Arc::new(MetricsRegistry::new()),
    };

    // Spawn background thermal poller (uses parse_pressure_level as the canonical parser).
    tokio::spawn(thermal_poll_task(
        Arc::clone(&state.thermal_tx),
        Arc::clone(&state.shutdown_tx),
        thermal_cancel,
    ));

    println!("sharecli serve listening on {url}");

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/assets/dashboard/ui/{*path}", get(crate::dashboard_assets::serve))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/config", get(config_handler))
        .route("/health/processes", get(health_processes_handler))
        .route("/metrics/prometheus", get(metrics_prometheus_handler))
        .route("/debug/pprof/profile", get(crate::pprof_http::profile_handler))
        .route("/ws", get(ws_handler))
        .layer(middleware::from_fn_with_state(state.clone(), http_observability_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), serve_rate_limit_middleware))
        .layer(middleware::from_fn_with_state(auth, serve_auth::require_bearer))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(serve_shutdown_signal(cancel.clone(), shutdown_rx))
        .await?;

    cancel.cancel();

    // Explicit drop for clarity; drop order would handle it anyway.
    drop(lock);
    info!("sharecli serve: stopped");
    audit_log::emit("serve_stop", json!({ "bind": bind }));
    crate::otel::shutdown();
    Ok(())
}
