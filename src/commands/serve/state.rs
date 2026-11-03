// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `AppState` shared between request handlers, middleware, and the
//! background thermal poller.
//!
//! `Clone` is derived so it can be handed into `Router::with_state` and
//! the per-middleware `from_fn_with_state` layers. All fields are
//! reference-counted because each request takes its own `AppState`
//! clone for the duration of the response.

use std::sync::Arc;

use tokio::sync::{broadcast, watch, RwLock};

use crate::config::Config;
use crate::health_check::HealthCheckStore;
use crate::http_red::HttpRedMetrics;
use crate::metrics::MetricsRegistry;
use crate::serve_rate_limit::ServeRateLimitState;

use super::pressure::ThermalEvent;

#[derive(Clone)]
pub(crate) struct AppState {
    /// Broadcast channel for thermal events.
    pub(crate) thermal_tx: Arc<broadcast::Sender<ThermalEvent>>,
    /// Set to `true` when a shutdown has been requested.
    pub(crate) shutdown_tx: Arc<watch::Sender<bool>>,
    /// Live config -- updated on hot-reload without restart.
    pub(crate) config: Arc<RwLock<Config>>,
    /// Shared health-check status for all monitored processes.
    pub(crate) health_store: HealthCheckStore,
    /// In-process HTTP RED metrics (Rate / Errors / Duration).
    pub(crate) http_red: Arc<HttpRedMetrics>,
    /// Optional sliding-window HTTP rate limiter (`None` = disabled).
    pub(crate) rate_limit: Arc<ServeRateLimitState>,
    /// OTel-style trace-context observability counters (inject / extract).
    pub(crate) metrics: Arc<MetricsRegistry>,
}
