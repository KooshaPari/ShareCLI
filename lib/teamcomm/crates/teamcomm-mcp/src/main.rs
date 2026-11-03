// SPDX-License-Identifier: Apache-2.0 OR MIT
//! MCP stdio server entry point. Reads JSON-RPC messages from stdin,
//! dispatches to MCP tool handlers (which forward to the teamcomm daemon
//! via `teamcomm-client`), writes responses to stdout.
//!
//! Wire protocol: each line is one JSON object. Supported methods:
//!   - "initialize" / "tools/list" → returns the static manifest
//!   - "tools/call"                → { name: "<tool>", arguments: {...} }
//!   - "ping"                      → returns {} (liveness check)
//! All other methods return JSON-RPC -32601 (method not found).
//!
//! Transport: STDIO per the MCP spec. Designed to be launched by an MCP
//! host (Codex/Claude/Copilot/etc.) as a child process.

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tracing::{error, info};

use teamcomm_mcp::{dispatch_tool_call, Manifest};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let manifest = Manifest::load_default().context("failed to load MCP manifest")?;
    info!(
        "teamcomm-mcp starting: {} tools from manifest version {}",
        manifest.tools.len(),
        manifest.version
    );

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line.context("failed to read stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    },
                });
                writeln!(stdout, "{}", resp).ok();
                stdout.flush().ok();
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let resp = match method {
            "initialize" | "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": manifest.name,
                        "version": manifest.version,
                    },
                    "capabilities": { "tools": {} },
                    "tools": manifest.tools,
                }
            }),
            "tools/call" => match dispatch_tool_call(req.clone()).await {
                Ok(envelope) => envelope,
                Err(e) => {
                    error!("dispatch failed: {:#}", e);
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("{e}"),
                        }
                    })
                }
            },
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {},
            }),
            "" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32600,
                    "message": "missing method",
                },
            }),
            other => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {other}"),
                },
            }),
        };

        writeln!(stdout, "{}", resp).context("stdout write failed")?;
        stdout.flush().ok();
    }

    Ok(())
}
