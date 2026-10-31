// SPDX-License-Identifier: Apache-2.0 OR MIT
//! WebSocket client for consuming the sharecli `/ws` streaming endpoint.
//!
//! # Usage
//!
//! ```no_run
//! use sharecli_ipc::ws_client::{SharecliClient, ClientMessage};
//! use futures_util::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = SharecliClient::new("ws://127.0.0.1:9000/ws");
//!     let mut stream = client.connect().await?;
//!     while let Some(msg) = stream.next().await {
//!         match msg? {
//!             ClientMessage::ProcessSnapshot(procs) => println!("procs: {}", procs.len()),
//!             ClientMessage::HealthUpdate(h) => println!("healthy={}", h.healthy),
//!             ClientMessage::ThermalEvent { level, message } => {
//!                 println!("thermal L{level}: {message}");
//!             }
//!             ClientMessage::Unknown(raw) => eprintln!("unknown frame: {raw}"),
//!         }
//!     }
//!     Ok(())
//! }
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use futures_util::stream::Stream;
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::handler::{HealthSnapshot, ProcessSummary};

// ---------------------------------------------------------------------------
// ClientMessage
// ---------------------------------------------------------------------------

/// A decoded message received from the sharecli WebSocket feed.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientMessage {
    /// A batch snapshot of all monitored processes.
    ProcessSnapshot(Vec<ProcessSummary>),
    /// A point-in-time health reading.
    HealthUpdate(Box<HealthSnapshot>),
    /// A thermal governor event.
    ThermalEvent { level: u8, message: String },
    /// A frame whose JSON shape was not recognised; the raw string is preserved.
    Unknown(String),
}

// Internal discriminated-union envelope used for JSON decoding.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Envelope {
    ProcessSnapshot { processes: Vec<ProcessSummary> },
    HealthUpdate { health: Box<HealthSnapshot> },
    ThermalEvent { level: u8, message: String },
}

impl ClientMessage {
    /// Parse a raw JSON text frame into a [`ClientMessage`].
    ///
    /// Unknown / unrecognised shapes fall through to [`ClientMessage::Unknown`]
    /// rather than returning an error, so the stream remains live.
    pub fn from_json(raw: &str) -> Self {
        match serde_json::from_str::<Envelope>(raw) {
            Ok(Envelope::ProcessSnapshot { processes }) => {
                ClientMessage::ProcessSnapshot(processes)
            }
            Ok(Envelope::HealthUpdate { health }) => ClientMessage::HealthUpdate(health),
            Ok(Envelope::ThermalEvent { level, message }) => {
                ClientMessage::ThermalEvent { level, message }
            }
            Err(_) => ClientMessage::Unknown(raw.to_owned()),
        }
    }
}

// ---------------------------------------------------------------------------
// SharecliClient
// ---------------------------------------------------------------------------

/// Connects to a running sharecli serve endpoint.
pub struct SharecliClient {
    url: String,
}

impl SharecliClient {
    /// Create a new client pointing at `url` (e.g. `"ws://127.0.0.1:9000/ws"`).
    pub fn new(url: &str) -> Self {
        Self { url: url.to_owned() }
    }

    /// Open a WebSocket connection and return a [`SharecliStream`].
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP/WS handshake fails.
    pub async fn connect(&self) -> Result<SharecliStream> {
        let (ws, _) = connect_async(&self.url)
            .await
            .map_err(|e| anyhow!("WS connect to {}: {e}", self.url))?;
        Ok(SharecliStream { inner: Box::pin(ws) })
    }
}

// ---------------------------------------------------------------------------
// SharecliStream
// ---------------------------------------------------------------------------

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// An async stream of decoded [`ClientMessage`]s from the sharecli WS feed.
///
/// Implements [`futures_util::Stream`]; drive it with `.next().await` or
/// any stream combinator.
pub struct SharecliStream {
    inner: Pin<Box<WsStream>>,
}

impl Stream for SharecliStream {
    type Item = Result<ClientMessage>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(anyhow!("WS error: {e}"))))
                }
                Poll::Ready(Some(Ok(msg))) => match msg {
                    Message::Text(text) => {
                        return Poll::Ready(Some(Ok(ClientMessage::from_json(text.as_str()))))
                    }
                    // Skip ping/pong/binary/close frames and continue polling.
                    _ => continue,
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_process_json() -> String {
        r#"{"type":"process_snapshot","processes":[{"pid":42,"name":"cargo","cmd":["cargo","test"],"memory_mb":128,"project":null,"harness":null,"start_time":1000}]}"#.to_owned()
    }

    fn make_health_json() -> String {
        r#"{"type":"health_update","health":{"managed_processes":3,"used_memory_mb":512,"total_memory_mb":16384,"healthy":true,"gate":{"thermal_pressure":"GREEN","detected_agents":0,"agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},"host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5},"pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,"healthy":true,"issues":[],"gate":{"thermal_pressure":"GREEN","detected_agents":0,"agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},"host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5}},"status":{"total_processes":2,"agents":[],"scanned":50,"watched":1,"gate":{"thermal_pressure":"GREEN","detected_agents":0,"agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},"host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5}}}}"#.to_owned()
    }

    fn make_legacy_health_json() -> String {
        r#"{"type":"health_update","health":{"managed_processes":3,"used_memory_mb":512,"total_memory_mb":16384,"healthy":true,"gate":{"thermal_pressure":"GREEN","detected_agents":0,"agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},"host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,"mem_rss_bytes":4,"load_1m":0.5}}}"#.to_owned()
    }

    fn make_thermal_json() -> String {
        r#"{"type":"thermal_event","level":2,"message":"CPU above 85C"}"#.to_owned()
    }

    fn make_unknown_json() -> String {
        r#"{"type":"something_new","data":{}}"#.to_owned()
    }

    fn make_garbage() -> String {
        "not json at all".to_owned()
    }

    #[test]
    fn deserialise_process_snapshot() {
        let msg = ClientMessage::from_json(&make_process_json());
        match msg {
            ClientMessage::ProcessSnapshot(procs) => {
                assert_eq!(procs.len(), 1);
                assert_eq!(procs[0].pid, 42);
                assert_eq!(procs[0].name, "cargo");
                assert_eq!(procs[0].memory_mb, 128);
            }
            other => panic!("expected ProcessSnapshot, got {other:?}"),
        }
    }

    #[test]
    fn deserialise_health_update() {
        let raw = make_health_json();
        let msg = ClientMessage::from_json(&raw);
        match msg {
            ClientMessage::HealthUpdate(h) => {
                assert_eq!(h.managed_processes, 3);
                assert_eq!(h.used_memory_mb, 512);
                assert!(h.healthy);
                assert_eq!(h.gate.gate_decision, "ADMIT");
                assert_eq!(h.host_watch.load_1m, 0.5);
                assert_eq!(h.pool.node_total, 2);
                assert!(h.pool.healthy);
                assert_eq!(h.status.scanned, 50);
                assert_eq!(h.status.watched, 1);
                let gate_pos = raw.find("\"gate\"").expect("gate key in health_update JSON");
                let host_pos =
                    raw.find("\"host_watch\"").expect("host_watch key in health_update JSON");
                let pool_pos =
                    raw.find("\"pool\"").expect("pool key in health_update JSON (AC-007.80)");
                let status_pos =
                    raw.find("\"status\"").expect("status key in health_update JSON (AC-007.80)");
                assert!(
                    gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
                    "health_update health MUST serialize gate → host_watch → pool → status (AC-007.80); got: {raw}"
                );
            }
            other => panic!("expected HealthUpdate, got {other:?}"),
        }
    }

    #[test]
    fn legacy_health_update_without_pool_status_yields_unknown() {
        let msg = ClientMessage::from_json(&make_legacy_health_json());
        assert!(
            matches!(msg, ClientMessage::Unknown(_)),
            "legacy health_update missing pool/status MUST NOT silently decode (AC-007.80)"
        );
    }

    #[test]
    fn deserialise_thermal_event() {
        let msg = ClientMessage::from_json(&make_thermal_json());
        match msg {
            ClientMessage::ThermalEvent { level, message } => {
                assert_eq!(level, 2);
                assert_eq!(message, "CPU above 85C");
            }
            other => panic!("expected ThermalEvent, got {other:?}"),
        }
    }

    #[test]
    fn unknown_type_falls_through() {
        let raw = make_unknown_json();
        let msg = ClientMessage::from_json(&raw);
        assert!(matches!(msg, ClientMessage::Unknown(_)), "unrecognised type must yield Unknown");
    }

    #[test]
    fn garbage_falls_through_to_unknown() {
        let raw = make_garbage();
        let msg = ClientMessage::from_json(&raw);
        assert!(matches!(msg, ClientMessage::Unknown(s) if s == raw));
    }
}
