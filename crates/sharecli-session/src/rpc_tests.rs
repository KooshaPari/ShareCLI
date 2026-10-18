// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::SurfaceCapabilities;

#[derive(Default)]
struct RecordingControl {
    sent: Mutex<Vec<(String, Vec<u8>)>>,
    resized: Mutex<Vec<(String, u16, u16)>>,
}

impl SurfaceControl for RecordingControl {
    fn send(&self, surface_id: &str, bytes: &[u8]) -> Result<()> {
        self.sent.lock().unwrap().push((surface_id.to_string(), bytes.to_vec()));
        Ok(())
    }

    fn read(&self, surface_id: &str, max_bytes: usize) -> Result<Vec<u8>> {
        assert_eq!(surface_id, "surface-1");
        Ok(b"terminal output"[..max_bytes.min(15)].to_vec())
    }

    fn resize(&self, surface_id: &str, rows: u16, cols: u16) -> Result<()> {
        self.resized.lock().unwrap().push((surface_id.to_string(), rows, cols));
        Ok(())
    }

    fn capabilities(&self, surface_id: &str) -> Result<SurfaceCapabilities> {
        assert_eq!(surface_id, "surface-1");
        Ok(SurfaceCapabilities {
            read: true,
            write: true,
            resize: true,
            layout: false,
            durable_pty: true,
        })
    }
}

struct OverrunControl;

impl SurfaceControl for OverrunControl {
    fn send(&self, _surface_id: &str, _bytes: &[u8]) -> Result<()> {
        Ok(())
    }

    fn read(&self, _surface_id: &str, max_bytes: usize) -> Result<Vec<u8>> {
        Ok(vec![0; max_bytes + 1])
    }

    fn resize(&self, _surface_id: &str, _rows: u16, _cols: u16) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self, _surface_id: &str) -> Result<SurfaceCapabilities> {
        Ok(SurfaceCapabilities {
            read: true,
            write: true,
            resize: true,
            layout: false,
            durable_pty: false,
        })
    }
}

async fn request(
    control: &Arc<RecordingControl>,
    id: u64,
    method: &str,
    params: Value,
) -> SurfaceResponse {
    let raw = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })
    .to_string();
    dispatch_surface(control.clone(), &raw).await
}

#[tokio::test]
async fn surface_send_passes_text_directly_to_control() {
    let control = Arc::new(RecordingControl::default());
    let response = request(
        &control,
        1,
        "surface.io.send",
        json!({"surface_id": "surface-1", "text": "printf 'not a shell'"}),
    )
    .await;

    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.result, Some(Value::Null));
    assert!(response.error.is_none());
    assert_eq!(
        *control.sent.lock().unwrap(),
        vec![("surface-1".to_string(), b"printf 'not a shell'".to_vec())]
    );
}

#[tokio::test]
async fn surface_list_reports_degraded_discovery_without_a_native_adapter() {
    let control = Arc::new(RecordingControl::default());
    let response = request(&control, 7, "surface.list", json!({})).await;
    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32000);
}

#[tokio::test]
async fn surface_send_accepts_an_explicit_byte_vector() {
    let control = Arc::new(RecordingControl::default());
    let response = request(
        &control,
        6,
        "surface.io.send",
        json!({"surface_id": "surface-1", "bytes": [0, 255, 10]}),
    )
    .await;

    assert_eq!(response.result, Some(Value::Null));
    assert_eq!(*control.sent.lock().unwrap(), vec![("surface-1".to_string(), vec![0, 255, 10])]);
}

#[tokio::test]
async fn surface_read_returns_a_typed_byte_vector() {
    let control = Arc::new(RecordingControl::default());
    let response =
        request(&control, 2, "surface.io.read", json!({"surface_id": "surface-1", "max_bytes": 8}))
            .await;

    assert_eq!(response.result, Some(json!({"bytes": b"terminal".to_vec()})));
    assert!(response.error.is_none());
}

#[tokio::test]
async fn surface_resize_forwards_dimensions_without_a_command_string() {
    let control = Arc::new(RecordingControl::default());
    let response = request(
        &control,
        3,
        "surface.io.resize",
        json!({"surface_id": "surface-1", "rows": 42, "cols": 120}),
    )
    .await;

    assert_eq!(response.result, Some(Value::Null));
    assert_eq!(*control.resized.lock().unwrap(), vec![("surface-1".to_string(), 42, 120)]);
}

#[tokio::test]
async fn surface_capabilities_are_returned_from_the_control_contract() {
    let control = Arc::new(RecordingControl::default());
    let response =
        request(&control, 4, "surface.io.capabilities", json!({"surface_id": "surface-1"})).await;

    assert_eq!(
        response.result,
        Some(json!({
            "read": true,
            "write": true,
            "resize": true,
            "layout": false,
            "durable_pty": true
        }))
    );
}

#[tokio::test]
async fn invalid_surface_params_return_json_rpc_invalid_params() {
    let control = Arc::new(RecordingControl::default());
    let response =
        request(&control, 5, "surface.io.resize", json!({"surface_id": "surface-1"})).await;

    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32602);
}

#[tokio::test]
async fn non_object_surface_params_return_json_rpc_invalid_params() {
    let control = Arc::new(RecordingControl::default());
    let response = request(&control, 8, "surface.list", json!(["not-an-object"])).await;

    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32602);
}

#[tokio::test]
async fn surface_send_rejects_oversized_payloads() {
    let control = Arc::new(RecordingControl::default());
    let response = request(
        &control,
        10,
        "surface.io.send",
        json!({
            "surface_id": "surface-1",
            "text": "x".repeat(MAX_SURFACE_SEND_BYTES + 1),
        }),
    )
    .await;

    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32602);
}

#[tokio::test]
async fn surface_read_rejects_provider_overruns() {
    let raw = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "surface.io.read",
        "params": {"surface_id": "surface-1", "max_bytes": 8}
    })
    .to_string();
    let response = dispatch_surface(Arc::new(OverrunControl), &raw).await;

    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, -32000);
}

#[tokio::test]
async fn surface_subscription_lifecycle_returns_ack_and_unsubscribe() {
    let control = Arc::new(RecordingControl::default());
    let events = Arc::new(SurfaceEventHub::new());
    let response = request_with_events(
        &control,
        &events,
        12,
        "surface.io.subscribe",
        json!({
            "surface_id": "surface-1",
            "from_seq": 1,
            "max_chunk_bytes": 1024,
            "queue_capacity": 4
        }),
    )
    .await;
    let ack: SurfaceSubscribeAck = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(ack.next_seq, 1);
    assert_eq!(ack.capabilities.max_chunk_bytes, 1024);
    assert_eq!(ack.capabilities.queue_capacity, 4);

    let response = request_with_events(
        &control,
        &events,
        13,
        "surface.io.unsubscribe",
        json!({"subscription_id": ack.subscription_id}),
    )
    .await;
    assert_eq!(response.result, Some(json!({"unsubscribed": true})));
    assert!(response.error.is_none());
}

#[test]
fn surface_events_keep_per_surface_sequence_and_wire_envelope() {
    let hub = SurfaceEventHub::new();
    let ack = hub.subscribe(SurfaceSubscribeRequest::new("surface-1")).unwrap();
    hub.publish_output("surface-1", b"one", Some("2026-08-02T03:00:00Z".into())).unwrap();
    hub.publish_output("surface-1", b"two", Some("2026-08-02T03:00:01Z".into())).unwrap();

    let events = hub.drain(ack.subscription_id, 8).unwrap();
    assert_eq!(events.iter().map(|event| event.params.seq).collect::<Vec<_>>(), vec![1, 2]);
    assert!(events.iter().all(|event| event.params.kind == SurfaceEventKind::Output));
    let wire = serde_json::to_value(&events[0]).unwrap();
    assert_eq!(wire["jsonrpc"], "2.0");
    assert_eq!(wire["method"], "surface.io.event");
    assert_eq!(wire["params"]["event_bytes_base64"], "b25l");
    assert_eq!(wire["params"]["seq"], 1);
}

#[test]
fn surface_subscription_overflow_emits_bounded_resync_marker() {
    let hub = SurfaceEventHub::new();
    let ack =
        hub.subscribe(SurfaceSubscribeRequest::new("surface-1").with_queue_capacity(2)).unwrap();
    hub.publish_output("surface-1", b"one", None).unwrap();
    hub.publish_output("surface-1", b"two", None).unwrap();
    hub.publish_output("surface-1", b"three", None).unwrap();

    let events = hub.drain(ack.subscription_id, 8).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].params.kind, SurfaceEventKind::Dropped);
    assert_eq!(events[0].params.dropped, Some(1));
    assert_eq!(events[0].params.resync_required, Some(true));
    assert!(events[1].params.seq > events[0].params.seq);
}

async fn request_with_events(
    control: &Arc<RecordingControl>,
    events: &Arc<SurfaceEventHub>,
    id: u64,
    method: &str,
    params: Value,
) -> SurfaceResponse {
    let raw = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })
    .to_string();
    dispatch_surface_with_events(control.clone(), events.clone(), &raw).await
}

#[cfg(unix)]
#[tokio::test]
async fn unix_server_round_trips_surface_json_rpc() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let socket = std::path::PathBuf::from(format!("/tmp/sharecli-sc-{}.sock", std::process::id()));
    let control = Arc::new(RecordingControl::default());
    let server_path = socket.clone();
    let server_control = control.clone();
    let server =
        tokio::spawn(async move { serve_surface_unix(&server_path, server_control).await });

    let mut stream = loop {
        match tokio::net::UnixStream::connect(&socket).await {
            Ok(stream) => break stream,
            Err(_error) if !server.is_finished() => tokio::task::yield_now().await,
            Err(error) => panic!("surface server failed before accepting connections: {error}"),
        }
    };
    stream
            .write_all(
                br#"{"jsonrpc":"2.0","id":9,"method":"surface.io.read","params":{"surface_id":"surface-1","max_bytes":8}}
"#,
            )
            .await
            .unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).await.unwrap();

    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap(),
        json!({"jsonrpc":"2.0","id":9,"result":{"bytes":b"terminal".to_vec()}})
    );
    server.abort();
    let _ = std::fs::remove_file(socket);
}

#[cfg(unix)]
#[tokio::test]
async fn unix_event_server_streams_bounded_output_notifications() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let socket = std::path::PathBuf::from(format!(
        "/tmp/sharecli-sc-events-{}-{}.sock",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let control = Arc::new(RecordingControl::default());
    let events = Arc::new(SurfaceEventHub::new());
    let server_path = socket.clone();
    let server_control = control.clone();
    let server_events = events.clone();
    let server = tokio::spawn(async move {
        serve_surface_unix_with_events(&server_path, server_control, server_events, None).await
    });

    let mut stream = loop {
        match tokio::net::UnixStream::connect(&socket).await {
            Ok(stream) => break stream,
            Err(_error) if !server.is_finished() => tokio::task::yield_now().await,
            Err(error) => panic!("event server failed before accepting connections: {error}"),
        }
    };
    stream
            .write_all(
                br#"{"jsonrpc":"2.0","id":1,"method":"surface.io.subscribe","params":{"surface_id":"surface-1","max_chunk_bytes":8,"queue_capacity":4}}
"#,
            )
            .await
            .unwrap();
    let mut reader = BufReader::new(stream);
    let mut ack = String::new();
    reader.read_line(&mut ack).await.unwrap();
    let ack: Value = serde_json::from_str(&ack).unwrap();
    let subscription_id = ack["result"]["subscription_id"].as_u64().unwrap();
    events.publish_output("surface-1", b"hello", None).unwrap();
    let mut event = String::new();
    tokio::time::timeout(std::time::Duration::from_secs(1), reader.read_line(&mut event))
        .await
        .unwrap()
        .unwrap();
    let event: Value = serde_json::from_str(&event).unwrap();
    assert_eq!(event["method"], "surface.io.event");
    assert_eq!(event["params"]["subscription_id"].as_u64(), Some(subscription_id));
    assert_eq!(event["params"]["event_bytes_base64"], "aGVsbG8=");
    server.abort();
    let _ = std::fs::remove_file(socket);
}
