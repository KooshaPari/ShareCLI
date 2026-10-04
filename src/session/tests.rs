//! Behavioural tests for the zmx and Ghostty surface adapters.
//!
//! The socket-facing tests use a real `UnixListener` bound to a temp-dir path
//! and answer with hand-written JSON-RPC frames, so the protocol encoding and
//! parsing are exercised end to end without any network or wall-clock wait.

use super::*;

// ---------------------------------------------------------------------------
// zmx adapters
// ---------------------------------------------------------------------------

#[test]
fn zmx_command_new_accepts_arrays_and_vectors() {
    let from_array = ZmxCommand::new("zmx", ["attach", "work"]);
    assert_eq!(from_array.program, "zmx");
    assert_eq!(from_array.args, vec!["attach".to_owned(), "work".to_owned()]);

    let from_vec = ZmxCommand::new(String::from("zmx"), vec![String::from("list")]);
    assert_eq!(from_vec.args, vec!["list".to_owned()]);
}

#[test]
fn zmx_attach_puts_the_name_before_the_command() {
    let adapter = ZmxSessionAdapter::new("zmx");
    let command = adapter.attach("work", &["cargo", "test"]);
    assert_eq!(command.program, "zmx");
    assert_eq!(command.args, vec!["attach", "work", "cargo", "test"]);
}

#[test]
fn zmx_attach_without_a_command_only_carries_the_name() {
    let command = ZmxSessionAdapter::new("zmx").attach("work", &[]);
    assert_eq!(command.args, vec!["attach", "work"]);
}

#[test]
fn zmx_send_carries_the_raw_input() {
    let command = ZmxSessionAdapter::new("zmx").send("work", "echo hi\n");
    assert_eq!(command.args, vec!["send", "work", "echo hi\n"]);
}

#[test]
fn zmx_tail_includes_the_line_flag_only_when_requested() {
    let adapter = ZmxSessionAdapter::new("zmx");
    assert_eq!(adapter.tail("work", Some(50)).args, vec!["tail", "--lines", "50", "work"]);
    assert_eq!(adapter.tail("work", None).args, vec!["tail", "work"]);
}

#[test]
fn zmx_history_includes_the_vt_flag_only_when_requested() {
    let adapter = ZmxSessionAdapter::new("zmx");
    assert_eq!(adapter.history("work", true).args, vec!["history", "--vt", "work"]);
    assert_eq!(adapter.history("work", false).args, vec!["history", "work"]);
}

#[test]
fn zmx_capabilities_follow_binary_presence_and_socket_probe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let binary = dir.path().join("zmx");
    std::fs::write(&binary, b"").expect("write stub binary");

    let adapter = ZmxSessionAdapter::new(binary.to_string_lossy().to_string());
    let probed = adapter.capabilities(true);
    assert!(probed.available);
    assert!(probed.durable_pty);
    assert!(probed.unix_socket, "socket support requires a successful probe");
    assert!(probed.history);

    let unprobed = adapter.capabilities(false);
    assert!(unprobed.available);
    assert!(!unprobed.unix_socket, "no probe means no claimed socket support");
}

#[test]
fn zmx_capabilities_are_absent_for_a_missing_binary() {
    let adapter = ZmxSessionAdapter::new("/nonexistent/sharecli-absent-zmx");
    let caps = adapter.capabilities(true);
    assert!(!caps.available);
    assert!(!caps.durable_pty);
    assert!(!caps.unix_socket, "an unavailable binary cannot have a socket");
    assert!(!caps.history);
}

#[cfg(unix)]
#[test]
fn command_available_resolves_bare_names_through_path() {
    assert!(command_available("sh"), "sh is on PATH in the test environment");
    assert!(!command_available("sharecli-definitely-absent-binary"));
}

#[cfg(unix)]
#[test]
fn execute_runs_the_command_without_a_shell() {
    let output = execute(&ZmxCommand::new("/bin/echo", ["hello", "world"])).expect("spawn echo");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello world");
}

// ---------------------------------------------------------------------------
// Ghostty capability gating
// ---------------------------------------------------------------------------

#[test]
fn ghostty_capabilities_default_to_no_control_socket() {
    let caps = GhosttyCapabilities::from_probe(true, true, true);
    assert!(caps.apple_events && caps.app_intents && caps.accessibility_readback);
    assert!(!caps.control_socket, "a stock install has no ShareCLI control socket");
    assert!(
        caps.clone().with_control_socket(true).control_socket,
        "with_control_socket overrides the probe default"
    );
    assert!(!caps.with_control_socket(false).control_socket);
}

#[test]
fn degraded_reason_prefers_the_native_api_gap() {
    let none = GhosttyCapabilities::from_probe(false, false, false);
    assert_eq!(GhosttyAdapter::degraded_reason(&none), Some("native surface API unavailable"));
    assert_eq!(
        GhosttyAdapter::degraded_reason(&none.with_control_socket(true)),
        None,
        "a live control socket is itself the native surface API"
    );
}

#[test]
fn degraded_reason_falls_back_to_the_rpc_gap() {
    let apple_events = GhosttyCapabilities::from_probe(true, false, false);
    assert_eq!(GhosttyAdapter::degraded_reason(&apple_events), Some("native RPC unavailable"));

    let app_intents = GhosttyCapabilities::from_probe(false, true, false);
    assert_eq!(GhosttyAdapter::degraded_reason(&app_intents), Some("native RPC unavailable"));
}

#[test]
fn degraded_reason_is_none_when_the_control_socket_is_present() {
    let caps = GhosttyCapabilities::from_probe(false, false, false).with_control_socket(true);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), None);
}

#[test]
fn control_client_exposes_its_socket_path() {
    let client = GhosttyControlClient::new("/tmp/sharecli.sock", Some("tkn".to_owned()));
    assert_eq!(client.socket(), Path::new("/tmp/sharecli.sock"));
}

// ---------------------------------------------------------------------------
// Surface event envelopes
// ---------------------------------------------------------------------------

#[test]
fn surface_event_envelope_round_trips_all_fields() {
    let envelope = SurfaceEventEnvelope {
        subscription_id: 9,
        surface_id: "srf-1".to_owned(),
        seq: 42,
        kind: SurfaceEventKind::Resize,
        timestamp: Some("2026-10-04T00:00:00Z".to_owned()),
        event_bytes_base64: Some("aGk=".to_owned()),
        dropped: Some(2),
        resync_required: Some(true),
    };
    let encoded = serde_json::to_string(&envelope).expect("serialize");
    let decoded: SurfaceEventEnvelope = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded, envelope);
}

#[test]
fn surface_event_envelope_tolerates_missing_optional_fields() {
    let decoded: SurfaceEventEnvelope = serde_json::from_value(json!({
        "subscription_id": 1,
        "surface_id": "srf-1",
        "seq": 0,
        "kind": "exit",
        "timestamp": null,
    }))
    .expect("deserialize minimal envelope");
    assert_eq!(decoded.kind, SurfaceEventKind::Exit);
    assert_eq!(decoded.event_bytes_base64, None);
    assert_eq!(decoded.dropped, None);
    assert_eq!(decoded.resync_required, None);
}

// ---------------------------------------------------------------------------
// JSON-RPC over a real Unix socket
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod rpc {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::mpsc;

    /// Bind a stub server that answers each of `replies` to one request line.
    ///
    /// Returns the socket path, the temp directory keeping it alive, and a
    /// handle that resolves once every connection has been served.
    fn stub_server(
        replies: Vec<Value>,
    ) -> (tempfile::TempDir, PathBuf, std::thread::JoinHandle<()>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind stub socket");
        let handle = std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut line = String::new();
                BufReader::new(stream.try_clone().expect("clone"))
                    .read_line(&mut line)
                    .expect("read request");
                stream.write_all(reply.to_string().as_bytes()).expect("write reply");
                stream.write_all(b"\n").expect("write newline");
                stream.flush().expect("flush reply");
            }
        });
        (dir, socket, handle)
    }

    /// A `SurfaceRecord` JSON value with no attached process evidence.
    fn surface_record_json(id: &str) -> Value {
        json!({
            "id": id,
            "terminal": "ghostty",
            "title": null,
            "cwd": "/work/proj",
            "process": null,
        })
    }

    fn valid_snapshot() -> LayoutSnapshot {
        LayoutSnapshot {
            id: "layout-1".to_owned(),
            terminal: "ghostty".to_owned(),
            captured_at: "2026-10-04T00:00:00Z".to_owned(),
            root: sharecli_session::LayoutNode::Pane { surface_id: "srf-1".to_owned() },
        }
    }

    #[test]
    fn request_returns_the_result_payload() {
        let (_dir, socket, server) = stub_server(vec![json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"ok": true},
        })]);
        let client = GhosttyControlClient::new(socket, None);

        let result = client.request("surface.ping", json!({})).expect("request succeeds");
        assert_eq!(result, json!({"ok": true}));
        server.join().expect("server joined");
    }

    #[test]
    fn request_reports_rpc_errors_with_the_method_name() {
        let (_dir, socket, server) = stub_server(vec![json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "unknown method"},
        })]);
        let client = GhosttyControlClient::new(socket, None);

        let err = client.request("surface.bogus", json!({})).expect_err("error frame is fatal");
        let text = err.to_string();
        assert!(text.contains("surface.bogus"), "names the failing method: {text}");
        assert!(text.contains("unknown method"), "carries the server message: {text}");
        server.join().expect("server joined");
    }

    #[test]
    fn request_without_a_result_field_yields_null() {
        let (_dir, socket, server) = stub_server(vec![json!({"jsonrpc": "2.0", "id": 1})]);
        let client = GhosttyControlClient::new(socket, None);

        assert_eq!(client.request("surface.ping", json!({})).expect("ok"), Value::Null);
        server.join().expect("server joined");
    }

    #[test]
    fn request_attaches_the_configured_token() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let (tx, rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut line = String::new();
            BufReader::new(stream.try_clone().expect("clone")).read_line(&mut line).expect("read");
            tx.send(line).expect("send captured request");
            stream.write_all(b"{\"id\":1,\"result\":null}\n").expect("reply");
        });

        let client = GhosttyControlClient::new(socket, Some("secret".to_owned()));
        client.request("surface.list", json!({})).expect("ok");

        let captured: Value = serde_json::from_str(&rx.recv().expect("captured")).expect("json");
        assert_eq!(captured["method"], "surface.list");
        assert_eq!(captured["token"], "secret");
        assert!(captured["id"].is_u64(), "requests carry a numeric id: {captured}");
        server.join().expect("server joined");
    }

    #[test]
    fn request_ids_are_unique_across_sequential_calls() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let (tx, rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut line = String::new();
                BufReader::new(stream.try_clone().expect("clone"))
                    .read_line(&mut line)
                    .expect("read");
                tx.send(line).expect("send");
                stream.write_all(b"{\"id\":1,\"result\":null}\n").expect("reply");
            }
        });

        let client = GhosttyControlClient::new(socket, None);
        client.request("surface.list", json!({})).expect("first");
        client.request("surface.list", json!({})).expect("second");

        let first: Value = serde_json::from_str(&rx.recv().expect("first")).expect("json");
        let second: Value = serde_json::from_str(&rx.recv().expect("second")).expect("json");
        assert_ne!(first["id"], second["id"], "ids must monotonically advance");
        server.join().expect("server joined");
    }

    #[test]
    fn typed_helpers_decode_their_payloads() {
        let (_dir, socket, server) = stub_server(vec![
            json!({"result": [surface_record_json("srf-1"), surface_record_json("srf-2")]}),
            json!({"result": {"read": true, "write": false, "resize": true, "layout": false, "durable_pty": true}}),
            json!({"result": {"bytes": "aGk=", "truncated": false}}),
            json!({"result": null}),
            json!({"result": null}),
        ]);
        let client = GhosttyControlClient::new(socket, None);

        let surfaces = client.list_surfaces().expect("list");
        assert_eq!(surfaces.len(), 2);
        assert_eq!(surfaces[0].id, "srf-1");
        assert_eq!(surfaces[1].cwd, Path::new("/work/proj"));
        assert!(surfaces[0].process.is_none());

        let caps = client.surface_capabilities("srf-1").expect("capabilities");
        assert!(caps.read && caps.durable_pty);
        assert!(!caps.write && !caps.layout);
        assert!(caps.resize);

        let read = client.read_surface("srf-1", 4096).expect("read");
        assert_eq!(read["bytes"], "aGk=");

        client.send_text("srf-1", "hello").expect("send_text");
        client.resize("srf-1", 40, 120).expect("resize");
        server.join().expect("server joined");
    }

    #[test]
    fn snapshot_layout_decodes_a_validated_snapshot() {
        let (_dir, socket, server) = stub_server(vec![json!({"result": valid_snapshot()})]);
        let client = GhosttyControlClient::new(socket, None);

        let snapshot = client.snapshot_layout().expect("snapshot");
        assert_eq!(snapshot, valid_snapshot());
        server.join().expect("server joined");
    }

    #[test]
    fn restore_layout_rejects_an_invalid_snapshot_before_connecting() {
        // The socket path deliberately does not exist: validation must fire
        // first, so this cannot fail for a connection reason.
        let client = GhosttyControlClient::new("/nonexistent/sharecli-absent.sock", None);
        let mut snapshot = valid_snapshot();
        snapshot.id = "  ".to_owned();

        let err = client.restore_layout(&snapshot).expect_err("invalid topology must be refused");
        assert!(err.to_string().contains("layout id"), "reports the validation failure: {err}");
    }

    #[test]
    fn restore_layout_decodes_the_report() {
        let (_dir, socket, server) = stub_server(vec![json!({
            "result": {
                "layout_id": "layout-1",
                "items": [{"surface_id": "srf-1", "restored": true, "detail": null}],
            },
        })]);
        let client = GhosttyControlClient::new(socket, None);

        let report = client.restore_layout(&valid_snapshot()).expect("restore");
        assert_eq!(report.layout_id, "layout-1");
        assert_eq!(report.items.len(), 1);
        assert!(report.items[0].restored);
        server.join().expect("server joined");
    }

    #[test]
    fn surface_adapter_trait_delegates_to_the_rpc_helpers() {
        let (_dir, socket, server) = stub_server(vec![
            json!({"result": [surface_record_json("srf-1")]}),
            json!({"result": {"read": true, "write": true, "resize": false, "layout": false, "durable_pty": false}}),
            json!({"result": valid_snapshot()}),
            json!({"result": {"layout_id": "layout-1", "items": []}}),
        ]);
        let client = GhosttyControlClient::new(socket, None);

        let discovered = SurfaceAdapter::discover(&client).expect("discover");
        assert_eq!(discovered.len(), 1);

        let caps = SurfaceAdapter::capabilities(&client, &discovered[0]).expect("capabilities");
        assert!(caps.read && caps.write);

        let snapshot = SurfaceAdapter::snapshot_layout(&client).expect("snapshot");
        assert_eq!(snapshot.id, "layout-1");

        let report = SurfaceAdapter::restore_layout(&client, &snapshot).expect("restore");
        assert!(report.items.is_empty());
        server.join().expect("server joined");
    }

    #[test]
    fn subscribe_rejects_out_of_range_limits() {
        let client = GhosttyControlClient::new("/nonexistent/sharecli-absent.sock", None);

        let err = client.subscribe_surface(None, None, 0, 16).err().expect("zero chunk bytes");
        assert!(err.to_string().contains("max_chunk_bytes"), "{err}");

        let err = client
            .subscribe_surface(None, None, MAX_EVENT_CHUNK_BYTES + 1, 16)
            .err()
            .expect("oversized chunk bytes");
        assert!(err.to_string().contains("max_chunk_bytes"), "{err}");

        let err = client.subscribe_surface(None, None, 1024, 0).err().expect("zero queue capacity");
        assert!(err.to_string().contains("queue_capacity"), "{err}");

        let err = client
            .subscribe_surface(None, None, 1024, MAX_EVENT_QUEUE_CAPACITY + 1)
            .err()
            .expect("oversized queue capacity");
        assert!(err.to_string().contains("queue_capacity"), "{err}");
    }

    #[test]
    fn subscribe_reports_a_server_error_frame() {
        let (_dir, socket, server) = stub_server(vec![json!({
            "error": {"code": -32600, "message": "subscription refused"},
        })]);
        let client = GhosttyControlClient::new(socket, None);

        let err = client.subscribe_surface(Some("srf-1"), None, 1024, 16).err().expect("refused");
        assert!(err.to_string().contains("subscribe failed"), "{err}");
        assert!(err.to_string().contains("subscription refused"), "{err}");
        server.join().expect("server joined");
    }

    /// Serve one subscription connection: ack, one event frame, then the
    /// unsubscribe reply. `token` must appear on the unsubscribe request.
    fn subscription_server(
        socket: &Path,
        token: Option<&str>,
        event: Value,
    ) -> std::thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket).expect("bind subscription socket");
        let token = token.map(str::to_owned);
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));

            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            assert!(request.contains("surface.io.subscribe"), "got: {request}");
            let sent: Value = serde_json::from_str(request.trim()).expect("subscribe json");
            assert_eq!(sent["params"]["max_chunk_bytes"], json!(1024));
            drop(sent);

            let ack = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "subscription_id": 7,
                    "next_seq": 1,
                    "capabilities": {"max_chunk_bytes": 1024, "queue_capacity": 16, "replay": true},
                },
            });
            stream.write_all(format!("{ack}\n").as_bytes()).expect("ack");
            stream.write_all(format!("{event}\n").as_bytes()).expect("event");

            let mut unsubscribe = String::new();
            reader.read_line(&mut unsubscribe).expect("read unsubscribe");
            assert!(unsubscribe.contains("surface.io.unsubscribe"), "got: {unsubscribe}");
            if let Some(token) = token {
                assert!(unsubscribe.contains(&token), "token must be echoed: {unsubscribe}");
            }
            stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{}}\n").expect("reply");
        })
    }

    #[test]
    fn subscribe_streams_events_and_unsubscribes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let event = json!({
            "jsonrpc": "2.0",
            "method": "surface.io.event",
            "params": {
                "subscription_id": 7,
                "surface_id": "srf-1",
                "seq": 3,
                "kind": "output",
                "timestamp": null,
                "event_bytes_base64": "aGk=",
            },
        });
        let server = subscription_server(&socket, Some("tkn"), event);
        let client = GhosttyControlClient::new(socket, Some("tkn".to_owned()));

        let mut subscription =
            client.subscribe_surface(Some("srf-1"), Some(0), 1024, 16).expect("subscribe");
        assert_eq!(subscription.id(), 7);

        let envelope = subscription.next_event().expect("event");
        assert_eq!(envelope.subscription_id, 7);
        assert_eq!(envelope.surface_id, "srf-1");
        assert_eq!(envelope.seq, 3);
        assert_eq!(envelope.kind, SurfaceEventKind::Output);
        assert_eq!(envelope.event_bytes_base64.as_deref(), Some("aGk="));

        subscription.unsubscribe().expect("unsubscribe");
        server.join().expect("server joined");
    }

    #[test]
    fn next_event_skips_unrelated_notifications() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"subscription_id\":1,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":16,\"replay\":false}}}\n",
                )
                .expect("ack");
            // A progress/other notification must be skipped, not returned.
            stream
                .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"surface.ping\",\"params\":{}}\n")
                .expect("ping");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"method\":\"surface.io.event\",\"params\":{\"subscription_id\":1,\"surface_id\":\"srf-9\",\"seq\":2,\"kind\":\"title\",\"timestamp\":\"now\"}}\n",
                )
                .expect("event");
            let mut unsubscribe = String::new();
            reader.read_line(&mut unsubscribe).expect("read unsubscribe");
            stream.write_all(b"{\"result\":{}}\n").expect("reply");
        });

        let client = GhosttyControlClient::new(socket, None);
        let mut subscription = client.subscribe_surface(None, None, 1024, 16).expect("subscribe");
        let envelope = subscription.next_event().expect("event");
        assert_eq!(envelope.kind, SurfaceEventKind::Title);
        assert_eq!(envelope.surface_id, "srf-9");
        assert_eq!(envelope.timestamp.as_deref(), Some("now"));
        subscription.unsubscribe().expect("unsubscribe");
        server.join().expect("server joined");
    }

    #[test]
    fn next_event_reports_a_closed_subscription() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        // Ack the subscription, then close the connection without an event.
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"subscription_id\":7,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":16,\"replay\":false}}}\n",
                )
                .expect("ack");
        });

        let client = GhosttyControlClient::new(socket, None);
        let mut subscription = client.subscribe_surface(None, None, 1024, 16).expect("subscribe");
        let err = subscription.next_event().expect_err("closed stream is an error");
        assert!(err.to_string().contains("closed"), "{err}");
        server.join().expect("server joined");
    }

    #[test]
    fn next_event_surfaces_an_error_notification() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"subscription_id\":2,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":16,\"replay\":false}}}\n",
                )
                .expect("ack");
            stream
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":5,\"error\":{\"message\":\"overflow\"}}\n")
                .expect("error frame");
            let mut unsubscribe = String::new();
            let _ = reader.read_line(&mut unsubscribe);
            let _ = stream.write_all(b"{\"result\":{}}\n");
        });

        let client = GhosttyControlClient::new(socket, None);
        let mut subscription = client.subscribe_surface(None, None, 1024, 16).expect("subscribe");
        let err = subscription.next_event().expect_err("server error frame is fatal");
        assert!(err.to_string().contains("live event RPC failed"), "{err}");
        assert!(err.to_string().contains("overflow"), "{err}");
        drop(subscription);
        server.join().expect("server joined");
    }

    #[test]
    fn unsubscribe_reports_a_server_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"subscription_id\":3,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":16,\"replay\":false}}}\n",
                )
                .expect("ack");
            let mut unsubscribe = String::new();
            reader.read_line(&mut unsubscribe).expect("read unsubscribe");
            stream
                .write_all(
                    b"{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"message\":\"not subscribed\"}}\n",
                )
                .expect("error reply");
        });

        let client = GhosttyControlClient::new(socket, None);
        let subscription = client.subscribe_surface(None, None, 1024, 16).expect("subscribe");
        let err = subscription.unsubscribe().expect_err("server rejected unsubscribe");
        assert!(err.to_string().contains("unsubscribe failed"), "{err}");
        assert!(err.to_string().contains("not subscribed"), "{err}");
        server.join().expect("server joined");
    }

    #[test]
    fn subscribe_rejects_an_unparsable_ack() {
        let (_dir, socket, server) = stub_server(vec![json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"subscription_id": "not-a-number"},
        })]);
        let client = GhosttyControlClient::new(socket, None);

        assert!(client.subscribe_surface(None, None, 1024, 16).is_err());
        server.join().expect("server joined");
    }

    #[test]
    fn subscribe_forwards_optional_filters_into_the_params() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let (tx, rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("read subscribe");
            tx.send(request).expect("send");
            let _ = stream.write_all(b"{\"result\":{\"subscription_id\":4,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":16,\"replay\":false}}}\n");
            let mut unsubscribe = String::new();
            let _ = reader.read_line(&mut unsubscribe);
            let _ = stream.write_all(b"{\"result\":{}}\n");
        });

        let client = GhosttyControlClient::new(socket, None);
        let subscription =
            client.subscribe_surface(Some("srf-7"), Some(11), 1024, 16).expect("subscribe");
        let sent: Value = serde_json::from_str(rx.recv().expect("captured").trim()).expect("json");
        assert_eq!(sent["method"], "surface.io.subscribe");
        assert_eq!(sent["params"]["surface_id"], "srf-7");
        assert_eq!(sent["params"]["from_seq"], 11);
        assert_eq!(sent["params"]["queue_capacity"], 16);
        drop(subscription.unsubscribe());
        server.join().expect("server joined");
    }
}
