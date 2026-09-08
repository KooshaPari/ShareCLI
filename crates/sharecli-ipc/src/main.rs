//! sharecli-ipc — JSON-RPC server
//!
//! Unix: listens on Unix socket `~/.local/share/sharecli/ipc.sock`
//! Windows: listens on TCP loopback `127.0.0.1:27182`
//! (or override via SHARECLI_IPC_SOCK or SHARECLI_IPC_ADDR env vars)
//!
//! Session database: default local data directory `sharecli/sessions.sqlite`.
//! Set SHARECLI_SESSION_DB to a nonempty file path to isolate durable session state.
//! Invalid database overrides fail startup without falling back to the default.
//!
//! Protocol: newline-delimited JSON (NDJSON).
//! Request:  `{"id": N, "method": "...", "params": {...}}`
//! Response: `{"id": N, "result": ..., "error": null}` or `{"id": N, "result": null, "error": "..."}`

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod handler;
mod log_buffer;

pub use handler::Handler;
use log_buffer::LogBufferLayer;

#[tokio::main]
async fn main() -> Result<()> {
    // Default filter: RUST_LOG (or "info"). Always funnel events into the
    // LogBufferLayer so the `log.tail` IPC arm can stream them to the tray.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(LogBufferLayer)
        .init();

    // Shared handler (holds ProcessPool + config)
    let handler = Arc::new(Handler::new().await?);

    #[cfg(unix)]
    {
        let sock_path = socket_path();

        // Remove stale socket from prior run
        if sock_path.exists() {
            std::fs::remove_file(&sock_path)?;
        }

        if let Some(parent) = sock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = tokio::net::UnixListener::bind(&sock_path)?;
        info!("sharecli-ipc listening on {}", sock_path.display());

        loop {
            let (stream, _) = listener.accept().await?;
            let h = handler.clone();
            tokio::spawn(async move {
                if let Err(e) = serve_unix_connection(stream, h).await {
                    error!("connection error: {e}");
                }
            });
        }
    }

    #[cfg(windows)]
    {
        let addr = ipc_addr();
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!("sharecli-ipc listening on {}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let h = handler.clone();
            tokio::spawn(async move {
                if let Err(e) = serve_tcp_connection(stream, h).await {
                    error!("connection error: {e}");
                }
            });
        }
    }
}

#[cfg(unix)]
async fn serve_unix_connection(
    stream: tokio::net::UnixStream,
    handler: Arc<Handler>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = handler.dispatch(trimmed).await;
        let mut payload = serde_json::to_string(&response)?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).await?;
    }

    Ok(())
}

#[cfg(windows)]
async fn serve_tcp_connection(stream: tokio::net::TcpStream, handler: Arc<Handler>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = handler.dispatch(trimmed).await;
        let mut payload = serde_json::to_string(&response)?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).await?;
    }

    Ok(())
}

pub fn socket_path() -> PathBuf {
    if let Ok(v) = std::env::var("SHARECLI_IPC_SOCK") {
        return PathBuf::from(v);
    }
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("sharecli").join("ipc.sock")
}

pub fn ipc_addr() -> String {
    std::env::var("SHARECLI_IPC_ADDR").unwrap_or_else(|_| "127.0.0.1:27182".to_string())
}

#[cfg(all(test, unix))]
mod acceptance_tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires SHARECLI_ACCEPTANCE_BRIDGE pointing to a pinned external consumer"]
    async fn current_resume_bridge_uses_actual_uds_handler() {
        let bridge = std::env::var("SHARECLI_ACCEPTANCE_BRIDGE")
            .expect("set SHARECLI_ACCEPTANCE_BRIDGE to the pinned resume-all bridge");
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("ipc.sock");
        let database = temp.path().join("sessions.sqlite");
        let handler = Arc::new(Handler::with_fixture_store(&database).unwrap());
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let handler = handler.clone();
                tokio::spawn(async move { serve_unix_connection(stream, handler).await.unwrap() });
            }
        });
        let result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("python3")
                .arg("-c")
                .arg(r#"
import importlib.util, json, os, socket
spec = importlib.util.spec_from_file_location('bridge', os.environ['SHARECLI_ACCEPTANCE_BRIDGE'])
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
    conn.settimeout(10)
    conn.connect(os.environ['SHARECLI_IPC_SOCK'])
    conn.sendall(b'{"id":4294967295,"method":"status.')
    conn.sendall(b'snapshot","params":{}}\n{"id":17,"method":"fixture.unknown"}\n')
    reader = conn.makefile('rb')
    snapshot = json.loads(reader.readline())
    assert snapshot['id'] == 4294967295 and snapshot['error'] is None
    assert isinstance(snapshot['result']['agents'], list)
    assert isinstance(snapshot['result']['total_processes'], int)
    rejected = json.loads(reader.readline())
    assert rejected['id'] == 17 and rejected['error'] and rejected['result'] is None
response = bridge.sharecli_call('status.snapshot', timeout=10)
assert response['error'] is None and isinstance(response['id'], int)
assert isinstance(response['result']['agents'], list)
rows = bridge.sharecli_session_list(timeout=10)
assert isinstance(rows, list)
for row in rows:
    assert {'pid', 'name', 'harness', 'state', 'mem_rss', 'mem_rss_bytes', 'session_id'} <= row.keys()
print('PASS: fragmented/pipelined NDJSON, correlation, error envelope, current bridge normalization')
"#)
                .env("SHARECLI_ACCEPTANCE_BRIDGE", bridge)
                .env("SHARECLI_IPC_SOCK", socket)
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .output().unwrap()
        }).await.unwrap();
        server.abort();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert!(database.exists());
        println!("{}", String::from_utf8_lossy(&result.stdout));
    }
}
