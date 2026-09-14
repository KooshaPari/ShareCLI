//! `sharecli serve` -- lock-guarded HTTP + WebSocket dashboard server.
//!
//! GET  /healthz  -- liveness probe (JSON)
//! GET  /readyz   -- readiness probe (JSON; 503 if shutdown requested)
//! WS   /ws       -- streams periodic ProcessSummary snapshots as JSON,
//!                   plus thermal pressure events when pressure changes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Serialize;
use serde_json::json;
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use sharecli_fleet::GateStatusSnapshot;
use tokio::sync::{broadcast, watch, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, warn, Instrument};

use crate::audit_log;
use crate::commands::proc::AgentProcSnapshot;
use crate::commands::{PoolJson, StatusJson};
use crate::config::Config;
use crate::config_watcher::ConfigWatcher;
use crate::error_envelope::ErrorEnvelope;
use crate::health_check::{HealthCheckScheduler, HealthCheckStore};
use crate::http_red::{render_http_red_metrics, HttpRedMetrics};
use crate::metrics::MetricsRegistry;
use crate::monitoring::HostResourceWatchJson;
use crate::notifier::Notifier;
#[cfg(test)]
use crate::runtime::ProcState;
use crate::runtime::ProcessPool;
use crate::serve_auth::{self, ServeAuth};
use crate::serve_lock::{decide, probe, Decision, OnConflict, ServeState};
use crate::serve_rate_limit::{is_probe_path, ServeRateLimit, ServeRateLimitState};
use crate::shutdown::serve_shutdown_signal;

// ---------------------------------------------------------------------------
// Pressure parsing (pure; tested without I/O)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Thermal event broadcast
// ---------------------------------------------------------------------------

/// Lightweight thermal event forwarded to WS clients.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ThermalEvent {
    ThermalWarning { pressure: u8 },
    ThermalCritical { pressure: u8 },
}

/// Parse a raw sysctl pressure integer into a [`ThermalLevel`].
///
/// This is a pure function with no I/O; it is unit-tested below.
pub fn parse_pressure_level(raw: u8) -> Option<ThermalLevel> {
    match raw {
        1 => Some(ThermalLevel::Green),
        2 => Some(ThermalLevel::Yellow),
        4 => Some(ThermalLevel::Red),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    /// Broadcast channel for thermal events.
    thermal_tx: Arc<broadcast::Sender<ThermalEvent>>,
    /// Set to `true` when a shutdown has been requested.
    shutdown_tx: Arc<watch::Sender<bool>>,
    /// Live config — updated on hot-reload without restart.
    config: Arc<RwLock<Config>>,
    /// Shared health-check status for all monitored processes.
    health_store: HealthCheckStore,
    /// In-process HTTP RED metrics (Rate / Errors / Duration).
    http_red: Arc<HttpRedMetrics>,
    /// Optional sliding-window HTTP rate limiter (`None` = disabled).
    rate_limit: Arc<ServeRateLimitState>,
    /// OTel-style trace-context observability counters (inject / extract).
    metrics: Arc<MetricsRegistry>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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
    let lock =
        crate::serve_lock::ServeLock::try_acquire("sharecli", url.clone())?.ok_or_else(|| {
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

// ---------------------------------------------------------------------------
// Background thermal poller
// ---------------------------------------------------------------------------

/// Read the raw sysctl pressure integer (platform-specific).
fn read_raw_pressure() -> anyhow::Result<u8> {
    // Delegate to ThermalGovernor for the sysctl call, then re-encode to u8
    // so that `parse_pressure_level` remains the single canonical parser.
    let gov = ThermalGovernor::new();
    let level = gov.poll()?;
    Ok(match level {
        ThermalLevel::Green => 1,
        ThermalLevel::Yellow => 2,
        ThermalLevel::Red => 4,
    })
}

async fn thermal_poll_task(
    tx: Arc<broadcast::Sender<ThermalEvent>>,
    shutdown_tx: Arc<watch::Sender<bool>>,
    cancel: CancellationToken,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("thermal poll task cancelled");
                return;
            }
            _ = interval.tick() => {}
        }
        let level = match read_raw_pressure() {
            Ok(raw) => parse_pressure_level(raw),
            Err(e) => {
                warn!("thermal poll error: {e}");
                continue;
            }
        };
        match level {
            Some(ThermalLevel::Red) => {
                info!("thermal pressure CRITICAL (4) -- broadcasting and initiating shutdown");
                let _ = tx.send(ThermalEvent::ThermalCritical { pressure: 4 });
                // Small delay so WS clients receive the message before connection drops.
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                let _ = shutdown_tx.send(true);
                return;
            }
            Some(ThermalLevel::Yellow) => {
                info!("thermal pressure WARNING (2) -- broadcasting");
                let _ = tx.send(ThermalEvent::ThermalWarning { pressure: 2 });
            }
            Some(ThermalLevel::Green) | None => {
                // No event needed for normal operation.
            }
        }
    }
}

/// Observability middleware: W3C `traceparent` extract/inject + HTTP RED metrics.
async fn http_observability_middleware(
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
async fn serve_rate_limit_middleware(
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
fn synthesize_traceparent() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let pid = u128::from(std::process::id());
    let trace_id = format!("{:032x}", nanos ^ (pid << 64));
    let span_id = format!("{:016x}", (nanos as u64) ^ (pid as u64));
    format!("00-{trace_id}-{span_id}-01")
}

const DASHBOARD_HTML: &str = include_str!("../dashboard.html");

#[instrument]
async fn dashboard() -> impl IntoResponse {
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
async fn healthz() -> impl IntoResponse {
    Json(healthz_json())
}

/// `GET /readyz` — readiness probe; 200 while serving, 503 once shutdown is requested.
#[instrument(skip(state))]
async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let (status, body) = readyz_response(*state.shutdown_tx.borrow());
    (status, Json(body))
}

/// `GET /config` — returns the current live config as JSON.
///
/// The value here reflects the last successful hot-reload; it updates
/// in-place whenever the config file is saved with valid TOML.
#[instrument(skip(state))]
async fn config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    match serde_json::to_value(cfg) {
        Ok(v) => Json(v).into_response(),
        Err(_) => ErrorEnvelope::internal().into_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut thermal_rx = state.thermal_tx.subscribe();
    let mut snapshot_interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

    loop {
        tokio::select! {
            // Periodic process snapshot
            _ = snapshot_interval.tick() => {
                let snapshot = match build_dashboard_ws_snapshot().await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("ws snapshot error: {e}");
                        continue;
                    }
                };
                let msg = match serde_json::to_string(&snapshot) {
                    Ok(s) => Message::Text(s.into()),
                    Err(e) => {
                        warn!("ws serialize error: {e}");
                        break;
                    }
                };
                if socket.send(msg).await.is_err() {
                    break;
                }
            }
            // Thermal event from background poller
            event = thermal_rx.recv() => {
                match event {
                    Ok(evt) => {
                        let msg = match serde_json::to_string(&evt) {
                            Ok(s) => Message::Text(s.into()),
                            Err(e) => {
                                warn!("ws thermal serialize error: {e}");
                                break;
                            }
                        };
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Skip missed events rather than disconnect.
                        warn!("ws thermal_rx lagged; skipping missed events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

/// `GET /health/processes` — returns health status of all monitored processes.
#[instrument(skip(state))]
async fn health_processes_handler(State(state): State<AppState>) -> impl IntoResponse {
    use std::collections::HashMap;

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

// ---------------------------------------------------------------------------
// Prometheus metrics handler
// ---------------------------------------------------------------------------

/// `GET /metrics/prometheus` — Prometheus text-format metrics for all tracked processes.
#[instrument(skip(state))]
async fn metrics_prometheus_handler(State(state): State<AppState>) -> impl IntoResponse {
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

/// Escape a Prometheus label value: backslash, double-quote, and newline must be escaped.
fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Render all sharecli process metrics in Prometheus text exposition format.
///
/// This is a pure function — no I/O — so it is straightforward to unit-test.
pub fn render_prometheus_metrics(
    processes: &[crate::runtime::ProcessInfo],
    health_map: &std::collections::HashMap<String, crate::health_check::HealthStatus>,
) -> String {
    let mut out = String::with_capacity(512);

    // -- sharecli_process_memory_mb ------------------------------------------
    out.push_str("# HELP sharecli_process_memory_mb Resident memory usage in MiB per process\n");
    out.push_str("# TYPE sharecli_process_memory_mb gauge\n");
    for p in processes {
        let name = escape_label_value(&p.name);
        out.push_str(&format!(
            "sharecli_process_memory_mb{{process=\"{}\"}} {}\n",
            name, p.memory_mb
        ));
    }

    // -- sharecli_process_up -------------------------------------------------
    out.push_str(
        "# HELP sharecli_process_up 1 if the process is considered healthy, 0 otherwise\n",
    );
    out.push_str("# TYPE sharecli_process_up gauge\n");
    for p in processes {
        let name = escape_label_value(&p.name);
        let up = if let Some(status) = health_map.get(&p.name) {
            if status.healthy {
                1u8
            } else {
                0u8
            }
        } else {
            // No health-check configured → process is running, treat as up.
            1u8
        };
        out.push_str(&format!("sharecli_process_up{{process=\"{}\"}} {}\n", name, up));
    }

    // -- sharecli_health_check_consecutive_failures --------------------------
    out.push_str("# HELP sharecli_health_check_consecutive_failures Number of consecutive health-check failures\n");
    out.push_str("# TYPE sharecli_health_check_consecutive_failures gauge\n");
    for (proc_name, status) in health_map.iter() {
        let name = escape_label_value(proc_name);
        out.push_str(&format!(
            "sharecli_health_check_consecutive_failures{{process=\"{}\"}} {}\n",
            name, status.consecutive_failures
        ));
    }

    // -- sharecli_health_check_status ----------------------------------------
    out.push_str("# HELP sharecli_health_check_status 1 if health check is passing, 0 otherwise\n");
    out.push_str("# TYPE sharecli_health_check_status gauge\n");
    for (proc_name, status) in health_map.iter() {
        let name = escape_label_value(proc_name);
        let val = if status.healthy { 1u8 } else { 0u8 };
        out.push_str(&format!("sharecli_health_check_status{{process=\"{}\"}} {}\n", name, val));
    }

    out
}

/// One managed process row in the dashboard WebSocket snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardProcessRow {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_mb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub start_time: u64,
}

/// Compact host-agent inventory for dashboard operator parity (AC-007.41).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardAgentSummary {
    pub scanned: usize,
    pub watched: usize,
    pub total_rss_bytes: u64,
    pub families: HashMap<String, usize>,
}

/// WebSocket operator envelope: gate → host_watch → pool → status → agents → processes
/// (AC-007.41 extended by AC-007.70 for pool/status IPC parity).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardWsSnapshot {
    pub gate: GateStatusSnapshot,
    pub host_watch: HostResourceWatchJson,
    pub pool: PoolJson,
    pub status: StatusJson,
    pub agents: DashboardAgentSummary,
    pub processes: Vec<DashboardProcessRow>,
}

/// Build the live dashboard WebSocket snapshot (parity with CLI/IPC pool + status envelopes).
pub async fn build_dashboard_ws_snapshot() -> anyhow::Result<DashboardWsSnapshot> {
    let managed_pool = ProcessPool::new();
    let (procs, pool_json, status_json) = tokio::join!(
        managed_pool.list(),
        crate::commands::build_pool_json(),
        crate::commands::build_status_json(),
    );
    let agent_snap = AgentProcSnapshot::capture()?;

    let mut families: HashMap<String, usize> = HashMap::new();
    let mut total_rss_bytes = 0u64;
    for row in &agent_snap.agents {
        *families.entry(row.family.clone()).or_insert(0) += 1;
        total_rss_bytes += row.mem_rss_bytes;
    }

    let processes = procs
        .iter()
        .map(|p| DashboardProcessRow {
            pid: p.pid,
            name: p.name.clone(),
            cmd: p.cmd.clone(),
            memory_mb: p.memory_mb,
            project: p.project.clone(),
            harness: p.harness.clone(),
            start_time: p.start_time,
        })
        .collect();

    Ok(DashboardWsSnapshot {
        gate: agent_snap.gate,
        host_watch: agent_snap.host_watch,
        pool: pool_json?,
        status: status_json?,
        agents: DashboardAgentSummary {
            scanned: agent_snap.scanned,
            watched: agent_snap.watched,
            total_rss_bytes,
            families,
        },
        processes,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use sharecli_fleet::thermal::ThermalLevel;

    use super::*;
    use crate::serve_lock::{decide, Decision, OnConflict, ServeInfo, ServeState};

    // --- serve_lock decision tests ---

    fn running_live(url: &str) -> ServeState {
        ServeState::Running {
            info: ServeInfo {
                pid: std::process::id(),
                service: "sharecli".into(),
                url: url.into(),
                started_at_unix: 1,
            },
            stale: false,
        }
    }

    fn running_stale() -> ServeState {
        ServeState::Running {
            info: ServeInfo {
                pid: u32::MAX,
                service: "sharecli".into(),
                url: "http://127.0.0.1:9000".into(),
                started_at_unix: 1,
            },
            stale: true,
        }
    }

    #[test]
    fn free_state_always_serves() {
        assert_eq!(decide(&ServeState::Free, OnConflict::Abort), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Attach), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Replace), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Prompt), Decision::Serve);
    }

    #[test]
    fn stale_running_serves_regardless_of_policy() {
        let stale = running_stale();
        assert_eq!(decide(&stale, OnConflict::Abort), Decision::Serve);
        assert_eq!(decide(&stale, OnConflict::Attach), Decision::Serve);
    }

    #[test]
    fn live_running_abort_policy_aborts() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Abort), Decision::Abort);
        assert_eq!(decide(&live, OnConflict::Prompt), Decision::Abort);
    }

    #[test]
    fn live_running_attach_policy_attaches() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Attach), Decision::Attach);
    }

    #[test]
    fn live_running_replace_policy_replaces() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Replace), Decision::Replace);
    }

    // --- parse_pressure_level unit tests ---

    #[test]
    fn parse_pressure_green() {
        assert_eq!(parse_pressure_level(1), Some(ThermalLevel::Green));
    }

    #[test]
    fn parse_pressure_yellow() {
        assert_eq!(parse_pressure_level(2), Some(ThermalLevel::Yellow));
    }

    #[test]
    fn parse_pressure_red() {
        assert_eq!(parse_pressure_level(4), Some(ThermalLevel::Red));
    }

    #[test]
    fn parse_pressure_unknown_returns_none() {
        assert_eq!(parse_pressure_level(0), None);
        assert_eq!(parse_pressure_level(3), None);
        assert_eq!(parse_pressure_level(5), None);
        assert_eq!(parse_pressure_level(255), None);
    }

    // --- ThermalEvent serialization tests ---

    #[test]
    fn thermal_event_warning_serializes() {
        let evt = ThermalEvent::ThermalWarning { pressure: 2 };
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"thermal_warning\""));
        assert!(s.contains("\"pressure\":2"));
    }

    #[test]
    fn thermal_event_critical_serializes() {
        let evt = ThermalEvent::ThermalCritical { pressure: 4 };
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"thermal_critical\""));
        assert!(s.contains("\"pressure\":4"));
    }

    // --- healthz / readyz JSON contract (no full server spin) ---

    #[test]
    fn healthz_json_is_ok() {
        let v = healthz_json();
        assert_eq!(v["status"], "ok");
    }

    /// Soft C08 live corpus: fixtures with `expect.health` must match `healthz_json`.
    #[test]
    fn corpus_health_fixtures_match_healthz() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/eval/corpus/scenarios");
        let mut checked = 0u32;
        for entry in std::fs::read_dir(&dir).expect("corpus scenarios dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read scenario");
            let data: serde_json::Value = serde_json::from_str(&raw).expect("parse scenario");
            let Some(health) = data.pointer("/expect/health").and_then(|v| v.as_str()) else {
                continue;
            };
            assert_eq!(
                healthz_json()["status"].as_str(),
                Some(health),
                "fixture {} expect.health mismatch",
                path.display()
            );
            checked += 1;
        }
        assert!(checked >= 1, "expected at least one corpus fixture with expect.health");
    }

    /// Soft C08 live corpus: fixtures with `expect.gate` must match thermal `gate_decision`.
    #[test]
    fn corpus_thermal_gate_fixtures_match_gate_decision() {
        use sharecli_thermal_tui::gate_decision;

        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/eval/corpus/scenarios");
        let mut checked = 0u32;
        for entry in std::fs::read_dir(&dir).expect("corpus scenarios dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read scenario");
            let data: serde_json::Value = serde_json::from_str(&raw).expect("parse scenario");
            let Some(expect_gate) = data.pointer("/expect/gate").and_then(|v| v.as_str()) else {
                continue;
            };
            let level = match data.get("thermal").and_then(|v| v.as_str()) {
                Some("red") => ThermalLevel::Red,
                Some("yellow") => ThermalLevel::Yellow,
                Some("green") | None => ThermalLevel::Green,
                Some(other) => panic!("fixture {}: unknown thermal {:?}", path.display(), other),
            };
            assert_eq!(
                gate_decision(level),
                expect_gate,
                "fixture {} expect.gate mismatch for {:?}",
                path.display(),
                level
            );
            if let Some(spawn_allowed) =
                data.pointer("/expect/spawn_allowed").and_then(|v| v.as_bool())
            {
                let allowed = expect_gate != "DENY";
                assert_eq!(
                    spawn_allowed,
                    allowed,
                    "fixture {} expect.spawn_allowed inconsistent with gate",
                    path.display()
                );
            }
            checked += 1;
        }
        assert!(checked >= 1, "expected at least one corpus fixture with expect.gate");
    }

    #[test]
    fn readyz_ok_while_serving() {
        let (status, body) = readyz_response(false);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
    }

    #[test]
    fn readyz_unavailable_after_shutdown() {
        let (status, body) = readyz_response(true);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unavailable");
    }

    // --- render_prometheus_metrics unit tests ---

    fn make_process(name: &str, memory_mb: u64) -> crate::runtime::ProcessInfo {
        crate::runtime::ProcessInfo {
            pid: 1234,
            name: name.to_string(),
            cmd: vec!["fake".to_string()],
            memory_mb,
            start_time: 0,
            cpu_percent: 0.0,
            project: None,
            harness: None,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        }
    }

    fn make_health(healthy: bool, failures: u32) -> crate::health_check::HealthStatus {
        crate::health_check::HealthStatus {
            healthy,
            consecutive_failures: failures,
            last_error: None,
            last_check: std::time::Instant::now(),
        }
    }

    #[test]
    fn prometheus_output_contains_required_metric_names() {
        let processes = vec![make_process("myapp", 256)];
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("myapp".to_string(), make_health(true, 0));
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(out.contains("sharecli_process_memory_mb"), "missing memory metric");
        assert!(out.contains("sharecli_process_up"), "missing process_up metric");
        assert!(
            out.contains("sharecli_health_check_consecutive_failures"),
            "missing failures metric"
        );
        assert!(out.contains("sharecli_health_check_status"), "missing health_check_status metric");
    }

    #[test]
    fn prometheus_process_up_reflects_health_status() {
        let processes = vec![make_process("svc", 64)];

        let mut healthy_map = std::collections::HashMap::new();
        healthy_map.insert("svc".to_string(), make_health(true, 0));
        let healthy_out = render_prometheus_metrics(&processes, &healthy_map);
        assert!(
            healthy_out.contains("sharecli_process_up{process=\"svc\"} 1"),
            "healthy process should have process_up=1"
        );

        let mut unhealthy_map = std::collections::HashMap::new();
        unhealthy_map.insert("svc".to_string(), make_health(false, 3));
        let unhealthy_out = render_prometheus_metrics(&processes, &unhealthy_map);
        assert!(
            unhealthy_out.contains("sharecli_process_up{process=\"svc\"} 0"),
            "unhealthy process should have process_up=0"
        );
    }

    #[test]
    fn prometheus_gauge_values_match_inputs() {
        let processes = vec![make_process("worker", 512)];
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("worker".to_string(), make_health(true, 7));
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(out.contains("sharecli_process_memory_mb{process=\"worker\"} 512"));
        assert!(out.contains("sharecli_health_check_consecutive_failures{process=\"worker\"} 7"));
    }

    #[test]
    fn prometheus_label_escaping_handles_special_chars() {
        // Process name with double-quote and backslash
        let processes = vec![make_process("my\"app\\test", 128)];
        let hmap = std::collections::HashMap::new();
        let out = render_prometheus_metrics(&processes, &hmap);
        // Escaped label value should appear; raw chars must not appear unescaped inside quotes
        assert!(
            out.contains(r#"process="my\"app\\test""#),
            "label value not properly escaped: {out}"
        );
    }

    #[test]
    fn prometheus_process_up_defaults_to_1_when_no_health_check() {
        // Process present in pool but NOT in health_store → should be up=1
        let processes = vec![make_process("orphan", 32)];
        let hmap = std::collections::HashMap::new(); // empty
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(
            out.contains("sharecli_process_up{process=\"orphan\"} 1"),
            "process without health check should default to up=1"
        );
    }

    // --- broadcast channel test ---

    #[tokio::test]
    async fn thermal_broadcast_delivers_to_subscriber() {
        let (tx, mut rx) = broadcast::channel::<ThermalEvent>(8);
        tx.send(ThermalEvent::ThermalWarning { pressure: 2 }).unwrap();
        let received = rx.recv().await.unwrap();
        matches!(received, ThermalEvent::ThermalWarning { pressure: 2 });
    }

    const HOST_WATCH_KEYS: [&str; 5] =
        ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

    fn ensure_test_config() {
        let _ = crate::config::init_global();
    }

    /// FR-007 / AC-007.41 — dashboard WS snapshot carries gate + host_watch operator envelope.
    #[tokio::test]
    async fn dashboard_ws_snapshot_includes_gate_and_host_watch() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

        assert!(
            v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
            "dashboard WS MUST include gate (AC-007.41); got: {v}"
        );
        let host = v.get("host_watch").expect("dashboard WS MUST include host_watch (AC-007.41)");
        for key in HOST_WATCH_KEYS {
            assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.41)");
        }
        assert!(v.get("agents").is_some(), "dashboard WS MUST include agents summary (AC-007.41)");
        assert!(v.get("processes").and_then(|p| p.as_array()).is_some());
    }

    /// FR-007 / AC-007.70 — dashboard WS snapshot carries pool + status IPC siblings.
    #[tokio::test]
    async fn dashboard_ws_snapshot_includes_pool_and_status() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

        let pool = v.get("pool").expect("dashboard WS MUST include pool (AC-007.70)");
        assert!(pool.get("node_total").is_some(), "pool MUST include node_total (AC-007.70)");
        assert!(pool.get("healthy").is_some(), "pool MUST include healthy (AC-007.70)");
        let status = v.get("status").expect("dashboard WS MUST include status (AC-007.70)");
        assert!(
            status.get("total_processes").is_some(),
            "status MUST include total_processes (AC-007.70)"
        );
        assert!(
            status.get("scanned").is_some() && status.get("watched").is_some(),
            "status MUST include scanned/watched (AC-007.70)"
        );
    }

    /// FR-007 / AC-007.41 — dashboard WS snapshot preserves gate → host_watch key ordering.
    #[tokio::test]
    async fn dashboard_ws_snapshot_gate_before_host_watch() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let gate_pos = raw.find("\"gate\"").expect("gate key in dashboard WS JSON");
        let host_pos = raw.find("\"host_watch\"").expect("host_watch key in dashboard WS JSON");
        assert!(
            gate_pos < host_pos,
            "dashboard WS MUST serialize gate before host_watch (AC-007.41); got: {raw}"
        );
    }

    /// FR-007 / AC-007.70 — dashboard WS snapshot preserves gate → host_watch → pool → status → agents order.
    #[tokio::test]
    async fn dashboard_ws_snapshot_operator_key_order() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let gate_pos = raw.find("\"gate\"").expect("gate key");
        let host_pos = raw.find("\"host_watch\"").expect("host_watch key");
        let pool_pos = raw.find("\"pool\"").expect("pool key (AC-007.70)");
        let status_pos = raw.find("\"status\"").expect("status key (AC-007.70)");
        let agents_pos = raw.find("\"agents\"").expect("agents key");
        let processes_pos = raw.find("\"processes\"").expect("processes key");
        assert!(
            gate_pos < host_pos
                && host_pos < pool_pos
                && pool_pos < status_pos
                && status_pos < agents_pos
                && agents_pos < processes_pos,
            "dashboard WS MUST serialize gate → host_watch → pool → status → agents → processes (AC-007.70); got: {raw}"
        );
    }

    // --- escape_label_value edge cases ---

    #[test]
    fn escape_label_value_empty_string() {
        assert_eq!(escape_label_value(""), "");
    }

    #[test]
    fn escape_label_value_no_special_chars() {
        assert_eq!(escape_label_value("hello world"), "hello world");
    }

    #[test]
    fn escape_label_value_newline_escaped() {
        assert_eq!(escape_label_value("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn escape_label_value_all_special_chars() {
        // Input has real backslash, double-quote, and newline chars
        let input = "back\\slash and \"quotes\" and newline\n";
        let escaped = escape_label_value(input);
        // backslash \ becomes \\
        assert!(escaped.contains("\\\\"), "backslash must be escaped to double-backslash");
        // double-quote " becomes \"
        assert!(escaped.contains("\\\""), "quotes must be escaped");
        // newline becomes \n (the two literal chars backslash + n)
        assert!(escaped.contains("\\n"), "newline must be escaped");
    }

    // --- render_prometheus_metrics edge cases ---

    #[test]
    fn prometheus_empty_processes_empty_health() {
        let out = render_prometheus_metrics(&[], &std::collections::HashMap::new());
        assert!(out.contains("sharecli_process_memory_mb"), "must have memory metric header");
        assert!(out.contains("sharecli_process_up"), "must have process_up metric header");
        assert!(out.contains("sharecli_health_check_consecutive_failures"));
        assert!(out.contains("sharecli_health_check_status"));
    }

    #[test]
    fn prometheus_health_only_no_processes() {
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("ghost-svc".to_string(), make_health(true, 0));
        let out = render_prometheus_metrics(&[], &hmap);
        // health-only entry should appear in failures and status metrics
        assert!(out.contains("sharecli_health_check_consecutive_failures{process=\"ghost-svc\"} 0"));
        assert!(out.contains("sharecli_health_check_status{process=\"ghost-svc\"} 1"));
    }

    // --- DashboardProcessRow serialization ---

    #[test]
    fn dashboard_process_row_serialization() {
        let row = DashboardProcessRow {
            pid: 42,
            name: "claude".into(),
            cmd: vec!["claude".into(), "serve".into()],
            memory_mb: 256,
            project: Some("myproject".into()),
            harness: Some("jcode".into()),
            start_time: 1_700_000_000,
        };
        let json = serde_json::to_string(&row).expect("serialize");
        assert!(json.contains("\"pid\":42"));
        assert!(json.contains("\"name\":\"claude\""));
        assert!(json.contains("\"memory_mb\":256"));
        assert!(json.contains("\"project\":\"myproject\""));
        assert!(json.contains("\"harness\":\"jcode\""));
    }

    #[test]
    fn dashboard_process_row_none_optional_fields_skipped() {
        let row = DashboardProcessRow {
            pid: 1,
            name: "x".into(),
            cmd: vec![],
            memory_mb: 0,
            project: None,
            harness: None,
            start_time: 0,
        };
        let json = serde_json::to_string(&row).expect("serialize");
        assert!(!json.contains("\"project\""), "None project must be skipped");
        assert!(!json.contains("\"harness\""), "None harness must be skipped");
    }

    // --- DashboardAgentSummary serialization ---

    #[test]
    fn dashboard_agent_summary_serialization() {
        let summary = DashboardAgentSummary {
            scanned: 10,
            watched: 5,
            total_rss_bytes: 1_000_000,
            families: HashMap::from([("claude".into(), 3), ("cursor".into(), 2)]),
        };
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(json.contains("\"scanned\":10"));
        assert!(json.contains("\"watched\":5"));
        assert!(json.contains("\"total_rss_bytes\":1000000"));
        assert!(json.contains("\"claude\""));
        assert!(json.contains("\"cursor\""));
    }

    // --- DashboardWsSnapshot serialization ---

    #[test]
    fn dashboard_ws_snapshot_serializes_all_fields() {
        let gate_snap = GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let snapshot = DashboardWsSnapshot {
            gate: gate_snap.clone(),
            host_watch: hw.clone(),
            pool: PoolJson {
                node_total: 0,
                node_idle: 0,
                bun_total: 0,
                bun_idle: 0,
                max_per_type: 0,
                healthy: true,
                issues: vec![],
                gate: gate_snap.clone(),
                host_watch: hw.clone(),
                status: None,
            },
            status: StatusJson {
                total_processes: 0,
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: gate_snap,
                host_watch: hw,
                pool: None,
                log_location: None,
            },
            agents: DashboardAgentSummary {
                scanned: 0,
                watched: 0,
                total_rss_bytes: 0,
                families: HashMap::new(),
            },
            processes: vec![],
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"gate\""));
        assert!(json.contains("\"host_watch\""));
        assert!(json.contains("\"pool\""));
        assert!(json.contains("\"status\""));
        assert!(json.contains("\"agents\""));
        assert!(json.contains("\"processes\""));
    }
}
