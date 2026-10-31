// SPDX-License-Identifier: Apache-2.0 OR MIT
//! WebSocket handler for the `serve` listener.
//!
//! `ws_handler` upgrades the HTTP request and `handle_ws` then streams
//! two kinds of messages:
//!   - Periodic `DashboardWsSnapshot` (via `build_dashboard_ws_snapshot`)
//!     every 500ms while the socket is open.
//!   - `ThermalEvent` values from the broadcast channel as the thermal
//!     poller reports them.
//!
//! If the broadcast channel reports `Lagged` we skip missed events
//! rather than disconnect (clients can re-fetch the next snapshot).

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use tokio::sync::broadcast;
use tracing::warn;

use super::dashboard::build_dashboard_ws_snapshot;
use super::state::AppState;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

pub async fn handle_ws(mut socket: WebSocket, state: AppState) {
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
