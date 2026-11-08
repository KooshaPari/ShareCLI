// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003
//! T-810 session coverage lift - ZmxSessionAdapter, GhosttyCapabilities, GhosttyControlClient.

use std::fs;

use sharecli::session::{
    GhosttyAdapter, GhosttyCapabilities, GhosttyControlClient, ZmxCommand, ZmxSessionAdapter,
};

/// FR-003 — ZmxCommand and ZmxSessionAdapter command builders plus capabilities probe.
#[test]
fn fr003_zmx_adapter_commands_and_capabilities() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Fake binary file so command_available returns true via Path::is_file branch.
    let fake_bin = tmp.path().join("fake-zmx");
    fs::write(&fake_bin, b"").expect("write fake bin");
    let fake_str = fake_bin.to_string_lossy().to_string();

    let adapter = ZmxSessionAdapter::new(fake_str.clone());
    let caps_available = adapter.capabilities(true);
    assert!(caps_available.available, "fake bin should be available");
    assert!(caps_available.durable_pty);
    assert!(caps_available.unix_socket, "probe_socket true + available");
    assert!(caps_available.history);

    let caps_no_socket = adapter.capabilities(false);
    assert!(caps_no_socket.available);
    assert!(!caps_no_socket.unix_socket);

    // Missing binary — available false
    let missing = ZmxSessionAdapter::new("nonexistent-zmx-binary-xyz-810");
    let caps_missing = missing.capabilities(true);
    assert!(!caps_missing.available);
    assert!(!caps_missing.durable_pty);
    assert!(!caps_missing.unix_socket);
    assert!(!caps_missing.history);

    // attach / send / tail / history builders
    let attach = adapter.attach("sess-1", &["--detach"]);
    assert_eq!(attach.program, fake_str);
    assert_eq!(attach.args, vec!["attach", "sess-1", "--detach"]);

    let send = adapter.send("sess-1", "hello world");
    assert_eq!(send.args, vec!["send", "sess-1", "hello world"]);

    let tail_no_lines = adapter.tail("sess-1", None);
    assert_eq!(tail_no_lines.args, vec!["tail", "sess-1"]);

    let tail_with = adapter.tail("sess-1", Some(50));
    assert_eq!(tail_with.args, vec!["tail", "--lines", "50", "sess-1"]);

    let hist_vt = adapter.history("sess-1", true);
    assert_eq!(hist_vt.args, vec!["history", "--vt", "sess-1"]);

    let hist_no_vt = adapter.history("sess-1", false);
    assert_eq!(hist_no_vt.args, vec!["history", "sess-1"]);

    // ZmxCommand::new generic constructor
    let cmd = ZmxCommand::new("prog", ["a", "b"]);
    assert_eq!(cmd.program, "prog");
    assert_eq!(cmd.args, vec!["a", "b"]);

    // execute with nonexistent binary yields Err
    let bad_cmd = ZmxCommand::new("nonexistent-binary-xyz-810", ["arg"]);
    let result = sharecli::session::execute(&bad_cmd);
    assert!(result.is_err(), "expected execute to fail for missing binary");
}

/// FR-003 — GhosttyCapabilities from_probe / with_control_socket and degraded_reason matrix.
#[test]
fn fr003_ghostty_capabilities_and_degraded_reason() {
    // All false -> native surface API unavailable (first branch)
    let caps = GhosttyCapabilities::from_probe(false, false, false);
    assert!(!caps.apple_events);
    assert!(!caps.app_intents);
    assert!(!caps.accessibility_readback);
    assert!(!caps.control_socket);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), Some("native surface API unavailable"));

    // Enable apple_events but still no control_socket -> native RPC unavailable
    let caps = GhosttyCapabilities::from_probe(true, false, false);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), Some("native RPC unavailable"));

    // App intents true, still no socket
    let caps = GhosttyCapabilities::from_probe(false, true, false);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), Some("native RPC unavailable"));

    // With control_socket true -> no degradation
    let caps = GhosttyCapabilities::from_probe(true, true, true).with_control_socket(true);
    assert!(caps.control_socket);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), None);

    // with_control_socket false after true then degraded
    let caps = GhosttyCapabilities::from_probe(true, true, true).with_control_socket(false);
    assert!(!caps.control_socket);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), Some("native RPC unavailable"));

    // All false but control_socket true manually -> no surface API unavailable, but still socket true means None
    let caps = GhosttyCapabilities::from_probe(false, false, false).with_control_socket(true);
    assert_eq!(GhosttyAdapter::degraded_reason(&caps), None);
}

/// FR-003 — GhosttyControlClient socket path and RPC failure on missing socket.
#[test]
fn fr003_ghostty_control_client_socket_and_request() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let socket = tmp.path().join("ghostty.sock");
    let client = GhosttyControlClient::new(socket.clone(), None);
    assert_eq!(client.socket(), socket.as_path());

    // With token variant stores correctly — socket path still same
    let client_tok = GhosttyControlClient::new(&socket, Some("tok-123".into()));
    assert_eq!(client_tok.socket(), socket.as_path());

    // Request to non-existent socket must fail (Unix: connect error; Windows: bail)
    let result = client.request("surface.list", serde_json::json!({}));
    assert!(result.is_err(), "expected RPC to fail without socket");

    let result = client.send_text("surf-1", "hello");
    assert!(result.is_err());

    let result = client.list_surfaces();
    assert!(result.is_err());

    let result = client.read_surface("surf-1", 1024);
    assert!(result.is_err());
}
