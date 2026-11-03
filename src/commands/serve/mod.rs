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
