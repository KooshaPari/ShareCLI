// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shell-free zmx and capability-gated Ghostty adapters.

use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicU64;
#[cfg(unix)]
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sharecli_session::{
    LayoutRestoreReport, LayoutSnapshot, SurfaceAdapter, SurfaceCapabilities, SurfaceEventKind,
    SurfaceRecord, MAX_EVENT_CHUNK_BYTES, MAX_EVENT_QUEUE_CAPACITY,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZmxCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl ZmxCommand {
    pub fn new<I, S>(program: impl Into<String>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { program: program.into(), args: args.into_iter().map(Into::into).collect() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZmxCapabilities {
    pub available: bool,
    pub durable_pty: bool,
    pub unix_socket: bool,
    pub history: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZmxSessionAdapter {
    binary: String,
}

impl ZmxSessionAdapter {
    pub fn new(binary: impl Into<String>) -> Self {
        Self { binary: binary.into() }
    }
    pub fn attach(&self, name: &str, command: &[&str]) -> ZmxCommand {
        let mut args = vec!["attach".to_owned(), name.to_owned()];
        args.extend(command.iter().map(|arg| (*arg).to_owned()));
        ZmxCommand::new(self.binary.clone(), args)
    }
    pub fn send(&self, name: &str, input: &str) -> ZmxCommand {
        ZmxCommand::new(self.binary.clone(), ["send", name, input])
    }
    pub fn tail(&self, name: &str, lines: Option<u32>) -> ZmxCommand {
        let mut args = vec!["tail".to_owned()];
        if let Some(lines) = lines {
            args.extend(["--lines".to_owned(), lines.to_string()]);
        }
        args.push(name.to_owned());
        ZmxCommand::new(self.binary.clone(), args)
    }
    pub fn history(&self, name: &str, vt: bool) -> ZmxCommand {
        let mut args = vec!["history".to_owned()];
        if vt {
            args.push("--vt".to_owned());
        }
        args.push(name.to_owned());
        ZmxCommand::new(self.binary.clone(), args)
    }
    pub fn capabilities(&self, probe_socket: bool) -> ZmxCapabilities {
        let available = command_available(&self.binary);
        ZmxCapabilities {
            available,
            durable_pty: available,
            unix_socket: available && probe_socket,
            history: available,
        }
    }
}

fn command_available(binary: &str) -> bool {
    Path::new(binary).is_file()
        || std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(binary).is_file()))
}

pub fn execute(command: &ZmxCommand) -> std::io::Result<std::process::Output> {
    Command::new(&command.program).args(&command.args).output()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhosttyCapabilities {
    pub apple_events: bool,
    pub app_intents: bool,
    pub accessibility_readback: bool,
    pub control_socket: bool,
}

impl GhosttyCapabilities {
    pub fn from_probe(apple_events: bool, app_intents: bool, accessibility_readback: bool) -> Self {
        Self { apple_events, app_intents, accessibility_readback, control_socket: false }
    }

    pub fn with_control_socket(mut self, available: bool) -> Self {
        self.control_socket = available;
        self
    }
}

/// Minimal JSON-RPC client for a ShareCLI-enabled Ghostty control socket.
///
/// A stock Ghostty install reports an unavailable socket; callers must then
/// use a degraded caster or zmx rather than pretending to have pane readback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhosttyControlClient {
    socket: PathBuf,
    token: Option<String>,
}

/// One server-originated event from a persistent surface subscription.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SurfaceEventEnvelope {
    pub subscription_id: u64,
    pub surface_id: String,
    pub seq: u64,
    pub kind: SurfaceEventKind,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub event_bytes_base64: Option<String>,
    #[serde(default)]
    pub dropped: Option<u64>,
    #[serde(default)]
    pub resync_required: Option<bool>,
}

/// Blocking reader/writer for a bounded live surface subscription.
#[cfg(unix)]
pub struct SurfaceSubscription {
    subscription_id: u64,
    token: Option<String>,
    writer: UnixStream,
    reader: BufReader<UnixStream>,
}

#[cfg(unix)]
impl SurfaceSubscription {
    pub fn id(&self) -> u64 {
        self.subscription_id
    }

    pub fn next_event(&mut self) -> anyhow::Result<SurfaceEventEnvelope> {
        loop {
            let mut line = String::new();
            let count = self.reader.read_line(&mut line)?;
            if count == 0 {
                anyhow::bail!("Ghostty live surface subscription closed")
            }
            let value: Value = serde_json::from_str(&line)?;
            if value.get("method").and_then(Value::as_str) != Some("surface.io.event") {
                if let Some(error) = value.get("error") {
                    anyhow::bail!("Ghostty live event RPC failed: {error}");
                }
                continue;
            }
            return Ok(serde_json::from_value(value["params"].clone())?);
        }
    }

    pub fn unsubscribe(mut self) -> anyhow::Result<()> {
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "surface.io.unsubscribe",
            "params": {"subscription_id": self.subscription_id},
        });
        let mut request = request;
        if let Some(token) = &self.token {
            request["token"] = Value::String(token.clone());
        }
        serde_json::to_writer(&mut self.writer, &request)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        let value: Value = serde_json::from_str(&line)?;
        if let Some(error) = value.get("error") {
            anyhow::bail!("Ghostty unsubscribe failed: {error}");
        }
        Ok(())
    }
}

#[cfg(unix)]
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

impl GhosttyControlClient {
    pub fn new(socket: impl Into<PathBuf>, token: Option<String>) -> Self {
        Self { socket: socket.into(), token }
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        #[cfg(unix)]
        {
            let mut stream = std::os::unix::net::UnixStream::connect(&self.socket)?;
            let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
            let mut request = json!({"id": id, "method": method, "params": params});
            if let Some(token) = &self.token {
                request["token"] = Value::String(token.clone());
            }
            serde_json::to_writer(&mut stream, &request)?;
            stream.write_all(b"\n")?;
            stream.flush()?;
            let mut response = String::new();
            BufReader::new(stream).read_line(&mut response)?;
            let response: Value = serde_json::from_str(&response)?;
            if let Some(error) = response.get("error") {
                anyhow::bail!("Ghostty RPC {method} failed: {error}");
            }
            Ok(response.get("result").cloned().unwrap_or(Value::Null))
        }
        #[cfg(not(unix))]
        {
            let _ = (method, params);
            anyhow::bail!("Ghostty control sockets require a Unix platform")
        }
    }

    pub fn send_text(&self, surface_id: &str, text: &str) -> anyhow::Result<()> {
        self.request("surface.io.send", json!({"surface_id": surface_id, "text": text}))?;
        Ok(())
    }

    pub fn list_surfaces(&self) -> anyhow::Result<Vec<SurfaceRecord>> {
        Ok(serde_json::from_value(self.request("surface.list", json!({}))?)?)
    }

    pub fn surface_capabilities(&self, surface_id: &str) -> anyhow::Result<SurfaceCapabilities> {
        Ok(serde_json::from_value(
            self.request("surface.io.capabilities", json!({"surface_id": surface_id}))?,
        )?)
    }

    pub fn read_surface(&self, surface_id: &str, max_bytes: usize) -> anyhow::Result<Value> {
        self.request("surface.io.read", json!({"surface_id": surface_id, "max_bytes": max_bytes}))
    }

    pub fn resize(&self, surface_id: &str, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.request(
            "surface.io.resize",
            json!({"surface_id": surface_id, "rows": rows, "cols": cols}),
        )?;
        Ok(())
    }

    /// Request a validated topology snapshot from the native Ghostty provider.
    pub fn snapshot_layout(&self) -> anyhow::Result<LayoutSnapshot> {
        serde_json::from_value(self.request("surface.layout.snapshot", json!({}))?)
            .map_err(Into::into)
    }

    /// Apply a durable topology through the native Ghostty provider.
    ///
    /// Validation happens before the request is sent, so malformed or
    /// duplicate surface trees never reach the app-side provider.
    pub fn restore_layout(&self, snapshot: &LayoutSnapshot) -> anyhow::Result<LayoutRestoreReport> {
        snapshot.validate()?;
        serde_json::from_value(
            self.request("surface.layout.restore", json!({"snapshot": snapshot}))?,
        )
        .map_err(Into::into)
    }

    #[cfg(unix)]
    pub fn subscribe_surface(
        &self,
        surface_id: Option<&str>,
        from_seq: Option<u64>,
        max_chunk_bytes: usize,
        queue_capacity: usize,
    ) -> anyhow::Result<SurfaceSubscription> {
        if !(1..=MAX_EVENT_CHUNK_BYTES).contains(&max_chunk_bytes) {
            anyhow::bail!("max_chunk_bytes must be between 1 and {MAX_EVENT_CHUNK_BYTES}");
        }
        if !(1..=MAX_EVENT_QUEUE_CAPACITY).contains(&queue_capacity) {
            anyhow::bail!("queue_capacity must be between 1 and {MAX_EVENT_QUEUE_CAPACITY}");
        }
        let stream = UnixStream::connect(&self.socket)?;
        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let mut params = json!({
            "max_chunk_bytes": max_chunk_bytes,
            "queue_capacity": queue_capacity,
        });
        if let Some(surface_id) = surface_id {
            params["surface_id"] = Value::String(surface_id.to_owned());
        }
        if let Some(from_seq) = from_seq {
            params["from_seq"] = Value::Number(from_seq.into());
        }
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "surface.io.subscribe",
            "params": params,
        });
        if let Some(token) = &self.token {
            request["token"] = Value::String(token.clone());
        }
        serde_json::to_writer(&mut writer, &request)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let response: Value = serde_json::from_str(&line)?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Ghostty subscribe failed: {error}");
        }
        let ack: sharecli_session::SurfaceSubscribeAck =
            serde_json::from_value(response.get("result").cloned().unwrap_or(Value::Null))?;
        Ok(SurfaceSubscription {
            subscription_id: ack.subscription_id,
            token: self.token.clone(),
            writer,
            reader,
        })
    }
}

impl SurfaceAdapter for GhosttyControlClient {
    fn capabilities(&self, surface: &SurfaceRecord) -> anyhow::Result<SurfaceCapabilities> {
        self.surface_capabilities(&surface.id)
    }

    fn discover(&self) -> anyhow::Result<Vec<SurfaceRecord>> {
        self.list_surfaces()
    }

    fn snapshot_layout(&self) -> anyhow::Result<LayoutSnapshot> {
        self.snapshot_layout()
    }

    fn restore_layout(&self, snapshot: &LayoutSnapshot) -> anyhow::Result<LayoutRestoreReport> {
        self.restore_layout(snapshot)
    }
}

pub struct GhosttyAdapter;

impl GhosttyAdapter {
    pub fn degraded_reason(caps: &GhosttyCapabilities) -> Option<&'static str> {
        (!caps.apple_events && !caps.app_intents && !caps.control_socket)
            .then_some("native surface API unavailable")
            .or_else(|| (!caps.control_socket).then_some("native RPC unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn zmx_command_new_collects_program_and_args() {
        let cmd = ZmxCommand::new("zmx", ["attach", "session-1"]);
        assert_eq!(cmd.program, "zmx");
        assert_eq!(cmd.args, vec!["attach", "session-1"]);
    }

    #[test]
    fn zmx_session_adapter_attach_emits_program_then_attach() {
        let adapter = ZmxSessionAdapter::new("zmx");
        let cmd = adapter.attach("pane-42", &["bash", "-c", "echo hi"]);
        assert_eq!(cmd.program, "zmx");
        assert_eq!(
            cmd.args,
            vec!["attach", "pane-42", "bash", "-c", "echo hi"]
        );
    }

    #[test]
    fn zmx_session_adapter_tail_omits_or_includes_lines_flag() {
        let adapter = ZmxSessionAdapter::new("zmx");
        // No lines -> no --lines flag, just trailing session name.
        let none_cmd = adapter.tail("pane-1", None);
        assert_eq!(none_cmd.program, "zmx");
        assert_eq!(none_cmd.args, vec!["tail".to_owned(), "pane-1".to_owned()]);
        // Some(lines) -> inserts --lines <N> before the session name.
        let some_cmd = adapter.tail("pane-2", Some(250));
        assert_eq!(
            some_cmd.args,
            vec![
                "tail".to_owned(),
                "--lines".to_owned(),
                "250".to_owned(),
                "pane-2".to_owned(),
            ]
        );
    }

    #[test]
    fn zmx_session_adapter_history_omits_or_includes_vt_flag() {
        let adapter = ZmxSessionAdapter::new("zmx");
        let plain = adapter.history("pane-3", false);
        assert_eq!(plain.args, vec!["history".to_owned(), "pane-3".to_owned()]);
        let vt = adapter.history("pane-3", true);
        assert_eq!(
            vt.args,
            vec!["history".to_owned(), "--vt".to_owned(), "pane-3".to_owned()]
        );
    }

    #[test]
    fn ghostty_capabilities_from_probe_starts_without_control_socket() {
        let caps = GhosttyCapabilities::from_probe(true, false, false);
        assert!(caps.apple_events);
        assert!(!caps.app_intents);
        assert!(!caps.accessibility_readback);
        assert!(!caps.control_socket, "control socket must default to false");
    }

    #[test]
    fn ghostty_capabilities_with_control_socket_toggles_flag() {
        let caps = GhosttyCapabilities::from_probe(false, false, false)
            .with_control_socket(true);
        assert!(caps.control_socket);
        let caps = GhosttyCapabilities::from_probe(false, false, false)
            .with_control_socket(false);
        assert!(!caps.control_socket);
    }

    #[test]
    fn ghostty_adapter_degraded_reason_for_each_missing_surface_capability() {
        // All three RPC channels disabled -> top reason applies.
        let none = GhosttyCapabilities::from_probe(false, false, false);
        assert_eq!(
            GhosttyAdapter::degraded_reason(&none),
            Some("native surface API unavailable")
        );
        // Only RPC channel missing -> second reason applies.
        let some_surface = GhosttyCapabilities::from_probe(true, true, true);
        assert_eq!(
            GhosttyAdapter::degraded_reason(&some_surface),
            Some("native RPC unavailable")
        );
        // All channels present -> no degraded reason.
        let full =
            GhosttyCapabilities::from_probe(true, true, true).with_control_socket(true);
        assert_eq!(GhosttyAdapter::degraded_reason(&full), None);
    }

    #[test]
    fn surface_event_envelope_serde_roundtrip_with_optional_fields() {
        let original = SurfaceEventEnvelope {
            subscription_id: 7,
            surface_id: "surface-9".to_owned(),
            seq: 42,
            kind: SurfaceEventKind::Output,
            timestamp: Some("2026-10-17T22:00:00Z".to_owned()),
            event_bytes_base64: None,
            dropped: None,
            resync_required: None,
        };
        let serialized = serde_json::to_string(&original).expect("serialize");
        // Optional fields with `#[serde(default)]` (no skip_serializing_if) are
        // emitted as `null` on the wire — confirms downstream readers see
        // explicit `null` rather than missing keys.
        assert!(serialized.contains("\"event_bytes_base64\":null"));
        assert!(serialized.contains("\"dropped\":null"));
        let parsed: SurfaceEventEnvelope =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed.subscription_id, 7);
        assert_eq!(parsed.surface_id, "surface-9");
        assert_eq!(parsed.seq, 42);
        assert_eq!(parsed.kind, SurfaceEventKind::Output);
        assert_eq!(parsed.timestamp.as_deref(), Some("2026-10-17T22:00:00Z"));
        assert!(parsed.event_bytes_base64.is_none());
        // JSON-shape round trip: serialize again and confirm byte-equivalence.
        let reserialized = serde_json::to_string(&parsed).expect("reserialize");
        assert_eq!(serialized, reserialized);

        // Lowercase enum rename_all: verify "kind": "dropped" parses back to Dropped.
        let raw = json!({
            "subscription_id": 1,
            "surface_id": "s",
            "seq": 0,
            "kind": "dropped",
            "timestamp": null,
        });
        let from_lower: SurfaceEventEnvelope =
            serde_json::from_value(raw).expect("lowercase kind");
        assert_eq!(from_lower.kind, SurfaceEventKind::Dropped);
    }
}
