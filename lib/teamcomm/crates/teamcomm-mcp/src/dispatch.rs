// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Dispatches incoming MCP `tools/call` envelopes to the appropriate
//! handler function in `handlers.rs`. The bridge forwards each tool call
//! through `teamcomm-client` to the teamcomm daemon over the Unix socket.

use crate::handlers::{self, ToolContext, ToolResult};
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Per-tool-call routing: maps MCP tool name → handler. Returns the JSON-RPC
/// response body for the call.
pub async fn route(tool: &str, args: Value) -> Result<ToolResult> {
    let mut ctx = ToolContext::new();

    let result = match tool {
        // Sessions
        "teamcomm_session_register" => handlers::register_session(&mut ctx, args).await,
        "teamcomm_session_deregister" => handlers::deregister_session(&mut ctx, args).await,
        "teamcomm_session_list" => handlers::list_sessions(&mut ctx, args).await,

        // Reservations
        "teamcomm_file_claim" => handlers::claim_file(&mut ctx, args).await,
        "teamcomm_file_release" => handlers::release_file(&mut ctx, args).await,
        "teamcomm_file_list_claims" => handlers::list_claims(&mut ctx, args).await,

        // Inbox
        "teamcomm_inbox_post" => handlers::post_message(&mut ctx, args).await,
        "teamcomm_inbox_read" => handlers::read_inbox(&mut ctx, args).await,

        // State
        "teamcomm_announce_focus" => handlers::announce_focus(&mut ctx, args).await,
        "teamcomm_set_status" => handlers::set_status(&mut ctx, args).await,

        // Discovery
        "teamcomm_discover_agents_for_path" => {
            handlers::discover_agents_for_path(&mut ctx, args).await
        }

        _ => Err(anyhow!("unknown tool: {}", tool)),
    };

    result
}

/// Top-level: takes an MCP `tools/call` envelope and returns the JSON-RPC body.
///
/// This is the entry-point used by `main.rs::run_server` on every incoming line
/// from the MCP client. The shape of the envelope is:
///
/// ```json
/// {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"...","arguments":{...}}}
/// ```
pub async fn dispatch_tool_call(envelope: Value) -> Result<Value> {
    let method = envelope
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing method"))?;

    if method != "tools/call" {
        return Ok(serde_json::json!({
            "jsonrpc": "2.0",
            "id": envelope.get("id").cloned().unwrap_or(Value::Null),
            "error": { "code": -32601, "message": format!("unknown method: {}", method) },
        }));
    }

    let id = envelope.get("id").cloned().unwrap_or(Value::Null);
    let params = envelope.get("params").cloned().unwrap_or(Value::Null);

    let tool = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing params.name"))?;
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    match route(tool, args).await {
        Ok(result) => Ok(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        })),
        Err(e) => Ok(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": format!("{:?}", e) },
        })),
    }
}

/// Error type for `dispatch_tool_call` — useful when callers want to fail-fast
/// without bubbling a JSON-RPC envelope. Most MCP server backends expect the
/// envelope-error path (above) so this is primarily for tests.
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("invalid envelope: {0}")]
    Invalid(String),
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("handler error: {0}")]
    Handler(String),
}

impl From<DispatchError> for Value {
    fn from(e: DispatchError) -> Value {
        serde_json::json!({ "error": format!("{}", e) })
    }
}
