// SPDX-License-Identifier: Apache-2.0 OR MIT
use sharecli::session::{
    GhosttyAdapter, GhosttyCapabilities, GhosttyControlClient, ZmxCommand, ZmxSessionAdapter,
};

#[test]
fn zmx_commands_are_shell_free() {
    let adapter = ZmxSessionAdapter::new("zmx");
    assert_eq!(
        adapter.attach("chat one", &["codex", "resume", "id"]),
        ZmxCommand::new("zmx", ["attach", "chat one", "codex", "resume", "id"])
    );
    assert_eq!(
        adapter.send("chat one", "hello\n"),
        ZmxCommand::new("zmx", ["send", "chat one", "hello\n"])
    );
    assert_eq!(
        adapter.tail("chat one", Some(40)),
        ZmxCommand::new("zmx", ["tail", "--lines", "40", "chat one"])
    );
    assert_eq!(
        adapter.history("chat one", true),
        ZmxCommand::new("zmx", ["history", "--vt", "chat one"])
    );
}

#[test]
fn missing_zmx_is_degraded() {
    let caps = ZmxSessionAdapter::new("definitely-not-installed-zmx").capabilities(false);
    assert!(!caps.available && !caps.durable_pty);
}

#[test]
fn ghostty_adapter_never_claims_private_rpc() {
    let caps = GhosttyCapabilities::from_probe(true, false, false);
    assert!(caps.apple_events && !caps.app_intents && !caps.control_socket);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), Some("native RPC unavailable"));
}

#[test]
fn ghostty_control_client_requires_configured_socket() {
    let client = GhosttyControlClient::new("/tmp/sharecli-no-ghostty.sock", Some("token".into()));
    assert!(client.request("surface.list", serde_json::json!({})).is_err());
    let caps = GhosttyCapabilities::from_probe(false, false, false).with_control_socket(true);
    assert!(GhosttyAdapter::degraded_reason(&caps).is_none());
}

#[cfg(unix)]
#[test]
fn ghostty_control_client_decodes_surface_inventory_and_capabilities() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let socket = std::env::temp_dir().join(format!("sharecli-ghostty-list-{suffix}.sock"));
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        for response in [
            r#"{"id":1,"result":[{"id":"ghostty:1","terminal":"ghostty","title":"agent","cwd":"/tmp","process":null}]}"#,
            r#"{"id":2,"result":{"read":true,"write":true,"resize":true,"layout":false,"durable_pty":false}}"#,
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });

    let client = GhosttyControlClient::new(&socket, None);
    assert_eq!(client.list_surfaces().unwrap()[0].id, "ghostty:1");
    assert!(client.surface_capabilities("ghostty:1").unwrap().read);
    server.join().unwrap();
    let _ = std::fs::remove_file(socket);
}

#[cfg(unix)]
#[test]
fn ghostty_control_client_round_trips_authenticated_io_request() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let socket = std::env::temp_dir().join(format!("sharecli-ghostty-{suffix}.sock"));
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
        let request: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["method"], "surface.io.send");
        assert_eq!(request["token"], "test-token");
        stream.write_all(b"{\"id\":1,\"result\":{\"accepted\":true}}\n").unwrap();
    });

    let client = GhosttyControlClient::new(&socket, Some("test-token".into()));
    client.send_text("ghostty:1", "hello\n").unwrap();
    server.join().unwrap();
    let _ = std::fs::remove_file(socket);
}

#[cfg(unix)]
#[test]
fn ghostty_control_client_consumes_live_event_and_unsubscribes() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let socket = std::env::temp_dir().join(format!("sharecli-ghostty-live-{suffix}.sock"));
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let subscribe: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(subscribe["method"], "surface.io.subscribe");
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"subscription_id\":1,\"next_seq\":1,\"capabilities\":{\"max_chunk_bytes\":1024,\"queue_capacity\":4,\"replay\":false}}}\n")
            .unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"surface.io.event\",\"params\":{\"subscription_id\":1,\"surface_id\":\"ghostty:1\",\"seq\":1,\"kind\":\"output\",\"timestamp\":null,\"event_bytes_base64\":\"aGk=\"}}\n")
            .unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        let unsubscribe: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(unsubscribe["method"], "surface.io.unsubscribe");
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"unsubscribed\":true}}\n")
            .unwrap();
    });

    let client = GhosttyControlClient::new(&socket, None);
    let mut subscription = client.subscribe_surface(Some("ghostty:1"), None, 1024, 4).unwrap();
    let event = subscription.next_event().unwrap();
    assert_eq!(event.subscription_id, 1);
    assert_eq!(event.surface_id, "ghostty:1");
    assert_eq!(event.seq, 1);
    assert_eq!(event.kind, sharecli_session::SurfaceEventKind::Output);
    subscription.unsubscribe().unwrap();
    server.join().unwrap();
    let _ = std::fs::remove_file(socket);
}

#[cfg(unix)]
#[test]
fn ghostty_control_client_rejects_invalid_subscription_limits_before_connecting() {
    let client = GhosttyControlClient::new("/tmp/sharecli-no-ghostty-live.sock", None);
    let chunk_error = client
        .subscribe_surface(None, None, 0, 1)
        .err()
        .expect("invalid chunk limit must fail")
        .to_string();
    assert!(chunk_error.contains("max_chunk_bytes"));
    let queue_error = client
        .subscribe_surface(None, None, 1024, 257)
        .err()
        .expect("invalid queue limit must fail")
        .to_string();
    assert!(queue_error.contains("queue_capacity"));
}
