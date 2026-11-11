//! Background thermal pressure poller.
//!
//! Reads the platform pressure integer (macOS / Linux) on a fixed interval
//! and broadcasts `ThermalEvent` values to WS clients. On Red it also
//! signals shutdown so the graceful-shutdown handler can drain in-flight
//! requests before the listener stops.

use std::sync::Arc;

use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use tokio::sync::{broadcast, watch};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::pressure::{parse_pressure_level, ThermalEvent};

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

pub async fn thermal_poll_task(
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
