// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::sync::Arc;

use anyhow::Result;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(test)]
use crate::SurfaceSubscribeAck;
use crate::DEFAULT_RECOVERY_MAX_AGE_SECONDS;
use crate::{
    LayoutRestoreReport, LayoutSnapshot, SessionService, SurfaceCapabilities, SurfaceEventError,
    SurfaceEventHub, SurfaceRecord, SurfaceSubscribeRequest,
};

#[cfg(unix)]
#[path = "rpc_transport.rs"]
mod transport;
#[cfg(unix)]
pub use transport::serve_surface_unix_with_token;

const JSON_RPC_VERSION: &str = "2.0";
const MAX_SURFACE_SEND_BYTES: usize = 64 * 1024;
const MAX_SURFACE_LINE_BYTES: usize = 1024 * 1024;
const MAX_SURFACE_READ_BYTES: usize = 1024 * 1024;
pub use crate::events::{
    SurfaceEventKind, SurfaceEventNotification, SurfaceEventParams,
    SurfaceSubscriptionCapabilities, MAX_EVENT_CHUNK_BYTES, MAX_EVENT_QUEUE_CAPACITY,
};
#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
#[derive(Debug, Serialize)]
pub struct Response {
    pub id: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
pub trait SurfaceControl: Send + Sync {
    fn list(&self) -> Result<Vec<SurfaceRecord>> {
        anyhow::bail!("surface discovery unavailable")
    }
    fn send(&self, surface_id: &str, bytes: &[u8]) -> Result<()>;
    fn read(&self, surface_id: &str, max_bytes: usize) -> Result<Vec<u8>>;
    fn resize(&self, surface_id: &str, rows: u16, cols: u16) -> Result<()>;
    fn capabilities(&self, surface_id: &str) -> Result<SurfaceCapabilities>;

    /// Capture the provider's current pane topology.
    ///
    /// The default is deliberately unavailable: a provider must opt into the
    /// layout contract rather than allowing an empty tree to masquerade as a
    /// successful snapshot.
    fn snapshot_layout(&self) -> Result<LayoutSnapshot> {
        anyhow::bail!("surface layout snapshot unavailable")
    }

    /// Apply a validated pane topology and return per-surface outcomes.
    ///
    /// Providers must implement this against their live surface tree. The
    /// ShareCLI transport validates the snapshot before crossing the boundary
    /// and never shells out to a terminal application.
    fn restore_layout(&self, snapshot: &LayoutSnapshot) -> Result<LayoutRestoreReport> {
        snapshot.validate()?;
        anyhow::bail!("surface layout restore unavailable")
    }
}
#[derive(Debug, Deserialize)]
struct SurfaceRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    token: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}
#[derive(Debug, Serialize)]
pub struct SurfaceResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}
#[derive(Deserialize)]
struct SurfaceIdParams {
    surface_id: String,
}
#[derive(Deserialize)]
struct SendParams {
    surface_id: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    bytes: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct ReadParams {
    surface_id: String,
    max_bytes: usize,
}

#[derive(Deserialize)]
struct ResizeParams {
    surface_id: String,
    rows: u16,
    cols: u16,
}

#[derive(Serialize)]
struct ReadResult {
    bytes: Vec<u8>,
}

pub async fn dispatch(service: Arc<SessionService>, line: &str) -> Response {
    let request = match serde_json::from_str::<Request>(line) {
        Ok(value) => value,
        Err(error) => {
            return Response {
                id: serde_json::Value::Null,
                result: None,
                error: Some(error.to_string()),
            }
        }
    };
    let outcome = match request.method.as_str() {
        "session.list" => service.list().map(|v| serde_json::to_value(v).unwrap_or_default()),
        "session.inspect" => request
            .params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("params.id is required"))
            .and_then(|id| {
                service.inspect(id).map(|v| serde_json::to_value(v).unwrap_or_default())
            }),
        "recovery.plan" => service
            .recovery_plan(Duration::seconds(DEFAULT_RECOVERY_MAX_AGE_SECONDS as i64))
            .map(|v| serde_json::to_value(v).unwrap_or_default()),
        method => Err(anyhow::anyhow!("unknown method: {method}")),
    };
    match outcome {
        Ok(result) => Response { id: request.id, result: Some(result), error: None },
        Err(error) => Response { id: request.id, result: None, error: Some(error.to_string()) },
    }
}

pub async fn dispatch_surface(control: Arc<dyn SurfaceControl>, line: &str) -> SurfaceResponse {
    dispatch_surface_internal(control, line, None).await
}

/// Dispatch a surface request while exposing the bounded live-event broker.
pub async fn dispatch_surface_with_events(
    control: Arc<dyn SurfaceControl>,
    events: Arc<SurfaceEventHub>,
    line: &str,
) -> SurfaceResponse {
    dispatch_surface_internal(control, line, Some(events)).await
}

async fn dispatch_surface_internal(
    control: Arc<dyn SurfaceControl>,
    line: &str,
    events: Option<Arc<SurfaceEventHub>>,
) -> SurfaceResponse {
    let raw = match serde_json::from_str::<Value>(line) {
        Ok(raw) => raw,
        Err(error) => return surface_error(Value::Null, -32700, format!("parse error: {error}")),
    };
    let params = match raw.get("params") {
        None => Value::Object(serde_json::Map::new()),
        Some(value) if value.is_object() => value.clone(),
        Some(_) => {
            let id = raw.get("id").cloned().unwrap_or(Value::Null);
            return surface_error(id, -32602, "params must be a JSON object".to_string());
        }
    };
    let request = match serde_json::from_value::<SurfaceRequest>(raw) {
        Ok(request) => request,
        Err(error) => {
            return surface_error(Value::Null, -32600, format!("invalid request: {error}"))
        }
    };
    if request.jsonrpc != JSON_RPC_VERSION {
        return surface_error(
            request.id.unwrap_or(Value::Null),
            -32600,
            "jsonrpc must be \"2.0\"".to_string(),
        );
    }

    let id = request.id.unwrap_or(Value::Null);
    let outcome = dispatch_surface_method_with_events(
        control.as_ref(),
        &request.method,
        params,
        events.as_deref(),
    );
    match outcome {
        Ok(result) => {
            SurfaceResponse { jsonrpc: JSON_RPC_VERSION, id, result: Some(result), error: None }
        }
        Err((code, message)) => surface_error(id, code, message),
    }
}

pub async fn dispatch_surface_with_token(
    control: Arc<dyn SurfaceControl>,
    line: &str,
    expected_token: Option<&str>,
) -> SurfaceResponse {
    if let Some(expected) = expected_token {
        let Ok(request) = serde_json::from_str::<SurfaceRequest>(line) else {
            return dispatch_surface(control, line).await;
        };
        if request.token.as_deref() != Some(expected) {
            return surface_error(
                request.id.unwrap_or(Value::Null),
                -32001,
                "invalid control token".into(),
            );
        }
    }
    dispatch_surface(control, line).await
}

pub async fn dispatch_surface_with_token_and_events(
    control: Arc<dyn SurfaceControl>,
    events: Arc<SurfaceEventHub>,
    line: &str,
    expected_token: Option<&str>,
) -> SurfaceResponse {
    if let Some(expected) = expected_token {
        let Ok(request) = serde_json::from_str::<SurfaceRequest>(line) else {
            return dispatch_surface_with_events(control, events, line).await;
        };
        if request.token.as_deref() != Some(expected) {
            return surface_error(
                request.id.unwrap_or(Value::Null),
                -32001,
                "invalid control token".into(),
            );
        }
    }
    dispatch_surface_with_events(control, events, line).await
}

fn dispatch_surface_method_with_events(
    control: &dyn SurfaceControl,
    method: &str,
    params: Value,
    events: Option<&SurfaceEventHub>,
) -> std::result::Result<Value, (i32, String)> {
    match method {
        "surface.list" => {
            let surfaces = control.list().map_err(control_error)?;
            serde_json::to_value(surfaces).map_err(control_error)
        }
        "surface.io.send" => {
            let params: SendParams = decode_params(params)?;
            let bytes = match (params.text, params.bytes) {
                (Some(text), None) => text.into_bytes(),
                (None, Some(bytes)) => bytes,
                _ => {
                    return Err((
                        -32602,
                        "exactly one of params.text or params.bytes is required".to_string(),
                    ))
                }
            };
            if bytes.len() > MAX_SURFACE_SEND_BYTES {
                return Err((
                    -32602,
                    format!("payload must not exceed {MAX_SURFACE_SEND_BYTES} bytes"),
                ));
            }
            control.send(&params.surface_id, &bytes).map_err(control_error)?;
            Ok(Value::Null)
        }
        "surface.io.read" => {
            let params: ReadParams = decode_params(params)?;
            if params.max_bytes > MAX_SURFACE_READ_BYTES {
                return Err((
                    -32602,
                    format!("max_bytes must not exceed {MAX_SURFACE_READ_BYTES}"),
                ));
            }
            let bytes =
                control.read(&params.surface_id, params.max_bytes).map_err(control_error)?;
            if bytes.len() > params.max_bytes {
                return Err((
                    -32000,
                    format!(
                        "surface provider returned {} bytes for a {} byte read",
                        bytes.len(),
                        params.max_bytes
                    ),
                ));
            }
            serde_json::to_value(ReadResult { bytes }).map_err(control_error)
        }
        "surface.io.resize" => {
            let params: ResizeParams = decode_params(params)?;
            if params.rows == 0 || params.cols == 0 {
                return Err((-32602, "rows and cols must be greater than zero".to_string()));
            }
            control.resize(&params.surface_id, params.rows, params.cols).map_err(control_error)?;
            Ok(Value::Null)
        }
        "surface.io.capabilities" => {
            let params: SurfaceIdParams = decode_params(params)?;
            let capabilities = control.capabilities(&params.surface_id).map_err(control_error)?;
            serde_json::to_value(capabilities).map_err(control_error)
        }
        "surface.layout.snapshot" => {
            if !params.as_object().is_some_and(|object| object.is_empty()) {
                return Err((-32602, "surface.layout.snapshot takes no params".to_string()));
            }
            serde_json::to_value(control.snapshot_layout().map_err(control_error)?)
                .map_err(control_error)
        }
        "surface.layout.restore" => {
            #[derive(Deserialize)]
            struct RestoreParams {
                snapshot: LayoutSnapshot,
            }
            let request: RestoreParams = decode_params(params)?;
            request.snapshot.validate().map_err(control_error)?;
            serde_json::to_value(control.restore_layout(&request.snapshot).map_err(control_error)?)
                .map_err(control_error)
        }
        "surface.io.subscribe" => {
            let Some(events) = events else {
                return Err((-32000, "live surface events unavailable".to_string()));
            };
            let request: SurfaceSubscribeRequest = decode_params(params)?;
            let ack = events.subscribe(request).map_err(event_error)?;
            serde_json::to_value(ack).map_err(control_error)
        }
        "surface.io.unsubscribe" => {
            let Some(events) = events else {
                return Err((-32000, "live surface events unavailable".to_string()));
            };
            #[derive(Deserialize)]
            struct UnsubscribeParams {
                subscription_id: u64,
            }
            let request: UnsubscribeParams = decode_params(params)?;
            let unsubscribed = events.unsubscribe(request.subscription_id).map_err(event_error)?;
            Ok(serde_json::json!({"unsubscribed": unsubscribed}))
        }
        _ => Err((-32601, format!("unknown method: {method}"))),
    }
}

fn event_error(error: SurfaceEventError) -> (i32, String) {
    (-32602, error.to_string())
}

fn decode_params<T: for<'de> Deserialize<'de>>(
    params: Value,
) -> std::result::Result<T, (i32, String)> {
    serde_json::from_value(params).map_err(|error| (-32602, format!("invalid params: {error}")))
}

fn control_error(error: impl std::fmt::Display) -> (i32, String) {
    (-32000, error.to_string())
}

#[rustfmt::skip]
fn surface_error(id: Value, code: i32, message: String) -> SurfaceResponse {
    SurfaceResponse { jsonrpc: JSON_RPC_VERSION, id, result: None, error: Some(RpcError { code, message }) }
}

#[cfg(unix)]
pub async fn serve_unix(path: &std::path::Path, service: Arc<SessionService>) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let service = service.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut payload) =
                    serde_json::to_string(&dispatch(service.clone(), &line).await)
                {
                    payload.push('\n');
                    if writer.write_all(payload.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        });
    }
}

#[cfg(unix)]
pub async fn serve_surface_unix(
    path: &std::path::Path,
    control: Arc<dyn SurfaceControl>,
) -> Result<()> {
    serve_surface_unix_with_token(path, control, None).await
}

/// Serve request/response RPC plus bounded server-originated events on one persistent socket.
/// The polling tick only drains already-published broker items; it never touches the provider.
#[cfg(unix)]
pub async fn serve_surface_unix_with_events(
    path: &std::path::Path,
    control: Arc<dyn SurfaceControl>,
    events: Arc<SurfaceEventHub>,
    expected_token: Option<String>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::time::{self, Duration};

    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let control = control.clone();
        let events = events.clone();
        let expected_token = expected_token.clone();
        tokio::spawn(async move {
            let (mut reader, mut writer) = stream.into_split();
            let mut input = Vec::new();
            let mut chunk = [0_u8; 16 * 1024];
            let mut subscriptions = Vec::new();
            let mut tick = time::interval(Duration::from_millis(25));
            loop {
                tokio::select! {
                    result = reader.read(&mut chunk) => {
                        let Ok(count) = result else { return };
                        if count == 0 { return; }
                        input.extend_from_slice(&chunk[..count]);
                        if input.len() > MAX_SURFACE_LINE_BYTES { return; }
                        while let Some(newline) = input.iter().position(|byte| *byte == b'\n') {
                            let Ok(line) = std::str::from_utf8(&input[..newline]) else { return; };
                            let request_value = serde_json::from_str::<Value>(line).ok();
                            let should_reply = serde_json::from_str::<Value>(line)
                                .ok()
                                .and_then(|request| request.as_object().map(|object| object.contains_key("id")))
                                .unwrap_or(false);
                            let response = dispatch_surface_with_token_and_events(
                                control.clone(),
                                events.clone(),
                                line,
                                expected_token.as_deref(),
                            ).await;
                            input.drain(..=newline);
                            if let Some(id) = response.result.as_ref()
                                .and_then(|result| result.get("subscription_id"))
                                .and_then(Value::as_u64)
                            {
                                subscriptions.push(id);
                            }
                            if request_value.as_ref().and_then(Value::as_object)
                                .and_then(|request| request.get("method"))
                                .and_then(Value::as_str) == Some("surface.io.unsubscribe")
                            {
                                if let Some(id) = request_value.as_ref()
                                    .and_then(|request| request.get("params"))
                                    .and_then(|params| params.get("subscription_id"))
                                    .and_then(Value::as_u64)
                                {
                                    subscriptions.retain(|subscription_id| *subscription_id != id);
                                }
                            }
                            if should_reply {
                                let mut payload = match serde_json::to_vec(&response) {
                                    Ok(payload) => payload,
                                    Err(_) => return,
                                };
                                payload.push(b'\n');
                                if writer.write_all(&payload).await.is_err() { return; }
                            }
                        }
                    }
                    _ = tick.tick() => {
                        for subscription_id in subscriptions.iter().copied() {
                            let queued = match events.drain(subscription_id, 32) {
                                Ok(events) => events,
                                Err(_) => continue,
                            };
                            for event in queued {
                                let mut payload = match serde_json::to_vec(&event) {
                                    Ok(payload) => payload,
                                    Err(_) => return,
                                };
                                payload.push(b'\n');
                                if writer.write_all(&payload).await.is_err() { return; }
                            }
                        }
                    }
                }
            }
        });
    }
}
#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
