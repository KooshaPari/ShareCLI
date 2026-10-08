//! Unix ShareCLI IPC client for resident process ownership.
//!
//! Do not fall back to a temporary ProcessPool: it cannot be shared by later
//! CLI invocations, so a successful "start" would be unmanageable.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Deserialize)]
pub struct ManagedSummary {
    pub pid: u32,
    pub project: Option<String>,
    pub harness: Option<String>,
}

fn socket_path() -> PathBuf {
    if let Ok(value) = std::env::var("SHARECLI_IPC_SOCK") {
        return PathBuf::from(value);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("sharecli")
        .join("ipc.sock")
}

pub async fn call(method: &str, params: Value) -> Result<Value> {
    let socket = socket_path();
    let stream = tokio::time::timeout(Duration::from_secs(3), UnixStream::connect(&socket))
        .await
        .context("ShareCLI IPC connection timed out")?
        .with_context(|| format!(
            "ShareCLI IPC supervisor is unavailable at {}; start sharecli-ipc first",
            socket.display()
        ))?;
    let (reader, mut writer) = stream.into_split();
    let payload = json!({"id": 1, "method": method, "params": params}).to_string();
    writer.write_all(payload.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    let mut line = String::new();
    tokio::time::timeout(
        Duration::from_secs(60),
        BufReader::new(reader).read_line(&mut line),
    )
    .await
    .context("ShareCLI IPC response timed out")??;
    anyhow::ensure!(!line.is_empty(), "ShareCLI IPC closed without a response");
    let response: Value = serde_json::from_str(&line)?;
    anyhow::ensure!(response["id"].as_u64() == Some(1), "ShareCLI IPC response ID mismatch");
    if let Some(error) = response["error"].as_str() {
        bail!("ShareCLI IPC {method}: {error}");
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}

pub async fn spawn(
    project: &str,
    harness: &str,
    cwd: &Path,
    args: &[String],
) -> Result<u32> {
    let result = call(
        "process.spawn",
        json!({"cmd": harness, "args": args, "cwd": cwd, "project": project, "harness": harness}),
    ).await?;
    let pid = result["pid"].as_u64().context("IPC spawn response missing PID")?;
    Ok(u32::try_from(pid).context("IPC spawn PID exceeds u32")?)
}

pub async fn list() -> Result<Vec<ManagedSummary>> {
    Ok(serde_json::from_value(call("process.list", json!({})).await?)?)
}

pub async fn kill(pid: u32) -> Result<bool> {
    let result = call("process.kill", json!({"pid": pid})).await?;
    result.as_bool().context("IPC kill response is not boolean")
}

pub async fn kill_all() -> Result<()> {
    let result = call("process.kill_all", json!({})).await?;
    anyhow::ensure!(result == Value::Bool(true), "IPC kill_all was not acknowledged");
    Ok(())
}
