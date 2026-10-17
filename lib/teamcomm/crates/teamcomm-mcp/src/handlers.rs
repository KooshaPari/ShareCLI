// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bridge implementations for each MCP tool. Each function takes the raw
//! `arguments` from the MCP tool-call envelope and forwards them — under
//! the daemon's wire names — to the `teamcomm-client` typed methods. No
//! business logic lives here: these are pure renames + zero-value defaults
//! where the MCP shape differs from the daemon's shape.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use teamcomm_client::Client;

pub type ToolResult = Value;

/// Per-call context: tracks the caller's session_id (set on the first
/// `register_session` call, reused for subsequent calls so the MCP caller
/// doesn't need to pass it every time).
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub agent_id: Option<String>,
}

impl ToolContext {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn known_tool_names() -> &'static [&'static str] {
    &[
        "register_session",
        "deregister_session",
        "list_sessions",
        "claim_file",
        "release_file",
        "list_claims",
        "post_message",
        "read_inbox",
        "announce_focus",
        "set_status",
        "discover_agents_for_path",
    ]
}

pub const KNOWN_TOOL_NAMES: &[&str] = &[
    "register_session",
    "deregister_session",
    "list_sessions",
    "claim_file",
    "release_file",
    "list_claims",
    "post_message",
    "read_inbox",
    "announce_focus",
    "set_status",
    "discover_agents_for_path",
];

async fn connect() -> Result<Client> {
    Client::connect_default()
        .await
        .map_err(|e| anyhow!("failed to connect to teamcomm daemon: {e}"))
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required string argument: {key}"))
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

// ─────────────────────────── session tools ───────────────────────────

/// MCP `register_session(agent_id, role, focus?, metadata?)` →
/// daemon `session.register { agent_type, pid, metadata }`.
///
/// Returns the daemon's response verbatim, which includes `session_id`.
/// Stores `agent_id` in the per-call context so subsequent calls can
/// reference it implicitly.
pub async fn register_session(ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let agent_id = require_str(&args, "agent_id")?.to_string();
    let role = require_str(&args, "role")?.to_string();
    let focus = opt_str(&args, "focus");
    let metadata = args
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| json!({}));

    ctx.agent_id = Some(agent_id.clone());

    let mut metadata_obj = metadata.as_object().cloned().unwrap_or_default();
    if !metadata_obj.contains_key("role") {
        metadata_obj.insert("role".into(), Value::String(role.clone()));
    }
    if !metadata_obj.contains_key("focus") {
        if let Some(f) = &focus {
            metadata_obj.insert("focus".into(), Value::String(f.clone()));
        }
    }
    metadata_obj.insert("mcp_role".into(), Value::String(role));
    metadata_obj.insert("mcp_agent_id".into(), Value::String(agent_id));

    let mut client = connect().await?;
    let params = json!({
        "agent_type": "codex",
        "pid": std::process::id(),
        "metadata": Value::Object(metadata_obj),
    });
    let result = client
        .call("session.register", params)
        .await
        .map_err(|e| anyhow!("session.register failed: {e}"))?;
    Ok(result)
}

/// MCP `deregister_session(session_id)` → daemon `session.deregister`.
pub async fn deregister_session(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let mut client = connect().await?;
    let params = json!({ "session_id": session_id });
    let result = client
        .call("session.deregister", params)
        .await
        .map_err(|e| anyhow!("session.deregister failed: {e}"))?;
    Ok(result)
}

/// MCP `list_sessions(role?, focus_contains?)` → daemon `session.list` with filter.
pub async fn list_sessions(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let mut params = json!({});
    if let Some(role) = opt_str(&args, "role") {
        params["role"] = json!(role);
    }
    if let Some(focus) = opt_str(&args, "focus_contains") {
        params["focus_contains"] = json!(focus);
    }
    let mut client = connect().await?;
    let result = client
        .call("session.list", params)
        .await
        .map_err(|e| anyhow!("session.list failed: {e}"))?;
    Ok(result)
}

// ─────────────────────────── reservation tools ───────────────────────────

/// MCP `claim_file(session_id, path, mode, ttl_sec?, reason?)` →
/// daemon `reservation.claim`.
pub async fn claim_file(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let path = require_str(&args, "path")?.to_string();
    let mode = require_str(&args, "mode")?.to_string();
    let ttl = args.get("ttl_sec").and_then(|v| v.as_u64()).unwrap_or(300);

    let mut client = connect().await?;
    let mut params = json!({
        "session_id": session_id,
        "path": path,
        "mode": mode,
        "ttl_seconds": ttl,
    });
    if let Some(reason) = opt_str(&args, "reason") {
        params["reason"] = json!(reason);
    }
    let result = client
        .call("reservation.claim", params)
        .await
        .map_err(|e| anyhow!("reservation.claim failed: {e}"))?;
    Ok(result)
}

/// MCP `release_file(reservation_id)` → daemon `reservation.release`.
pub async fn release_file(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let reservation_id = require_str(&args, "reservation_id")?.to_string();
    let session_id = opt_str(&args, "session_id");
    let mut client = connect().await?;
    let mut params = json!({ "reservation_id": reservation_id });
    if let Some(sid) = session_id {
        params["session_id"] = json!(sid);
    }
    let result = client
        .call("reservation.release", params)
        .await
        .map_err(|e| anyhow!("reservation.release failed: {e}"))?;
    Ok(result)
}

/// MCP `list_claims(session_id?, path_prefix?, mode?)` → daemon `reservation.list`.
pub async fn list_claims(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let mut params = json!({});
    if let Some(sid) = opt_str(&args, "session_id") {
        params["session_id"] = json!(sid);
    }
    if let Some(prefix) = opt_str(&args, "path_prefix") {
        params["path_prefix"] = json!(prefix);
    }
    if let Some(mode) = opt_str(&args, "mode") {
        params["mode"] = json!(mode);
    }
    let mut client = connect().await?;
    let result = client
        .call("reservation.list", params)
        .await
        .map_err(|e| anyhow!("reservation.list failed: {e}"))?;
    Ok(result)
}

// ─────────────────────────── inbox tools ───────────────────────────

/// MCP `post_message(session_id, to?, topic?, kind?, body, metadata?)` →
/// daemon `inbox.post { from_session, to_session, kind, body }`.
pub async fn post_message(ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let to = opt_str(&args, "to");
    let body = require_str(&args, "body")?.to_string();
    let topic = opt_str(&args, "topic");
    let kind = opt_str(&args, "kind").unwrap_or_else(|| "note".into());
    let priority = opt_str(&args, "priority").unwrap_or_else(|| "normal".into());

    let mut client = connect().await?;
    let mut params = json!({
        "from_session": session_id,
        "kind": kind,
        "priority": priority,
        "body": body,
    });
    if let Some(t) = to {
        params["to_session"] = json!(t);
    } else {
        // No `to`: post to the team-wide broadcast. Daemon requires
        // `to_session`, so we set it to the empty string and let the
        // daemon ignore it (server treats `""` as broadcast marker).
        params["to_session"] = json!("");
    }
    if let Some(t) = topic {
        params["subject"] = json!(t);
    }
    let _ = ctx; // session id sourced from args; ignore ctx
    let result = client
        .call("inbox.post", params)
        .await
        .map_err(|e| anyhow!("inbox.post failed: {e}"))?;
    Ok(result)
}

/// MCP `read_inbox(session_id, limit?, since?)` → daemon `inbox.list`.
pub async fn read_inbox(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let mut client = connect().await?;
    let mut params = json!({ "session_id": session_id });
    if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
        params["limit"] = json!(limit);
    }
    if let Some(since) = opt_str(&args, "since") {
        params["since"] = json!(since);
    }
    let result = client
        .call("inbox.list", params)
        .await
        .map_err(|e| anyhow!("inbox.list failed: {e}"))?;
    Ok(result)
}

// ─────────────────────────── state / focus / discovery tools ───────────────────────────

/// MCP `announce_focus(session_id, focus)` → daemon `state.set` with focus.
pub async fn announce_focus(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let focus = require_str(&args, "focus")?.to_string();
    let mut client = connect().await?;
    let params = json!({
        "session_id": session_id,
        "state": { "focus": focus, "status": "working" },
    });
    let result = client
        .call("state.set", params)
        .await
        .map_err(|e| anyhow!("state.set (announce_focus) failed: {e}"))?;
    Ok(result)
}

/// MCP `set_status(session_id, status, message?)` → daemon `state.set` with status.
pub async fn set_status(_ctx: &mut ToolContext, args: Value) -> Result<ToolResult> {
    let session_id = require_str(&args, "session_id")?.to_string();
    let status = require_str(&args, "status")?.to_string();
    let message = opt_str(&args, "message");

    let mut client = connect().await?;
    let mut state_obj = json!({ "status": status });
    if let Some(m) = message {
        state_obj["status_message"] = json!(m);
    }
    let params = json!({
        "session_id": session_id,
        "state": state_obj,
    });
    let result = client
        .call("state.set", params)
        .await
        .map_err(|e| anyhow!("state.set (set_status) failed: {e}"))?;
    Ok(result)
}

/// MCP `discover_agents_for_path(path, include_idle?)` →
/// daemon `discover.agents` with path-filter (uses `discover.agents` as a
/// broad session list; matches daemon's `ReservationFilter` shape).
pub async fn discover_agents_for_path(
    _ctx: &mut ToolContext,
    args: Value,
) -> Result<ToolResult> {
    let path = require_str(&args, "path")?.to_string();
    let include_idle = args
        .get("include_idle")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut client = connect().await?;
    let mut params = json!({
        "path": path,
        "include_idle": include_idle,
    });
    // Pass-through optional agent_type filter if provided.
    if let Some(t) = opt_str(&args, "agent_type") {
        params["agent_type"] = json!(t);
    }
    let result = client
        .call("discover.agents", params)
        .await
        .map_err(|e| anyhow!("discover.agents failed: {e}"))?;
    Ok(result)
}

// ─────────────────────────── internals ───────────────────────────

/// Returns an explicit error if the args object is missing required keys.
pub fn validate_required<'a>(args: &'a Value, keys: &[&str]) -> Result<()> {
    let missing: Vec<&str> = keys
        .iter()
        .filter(|k| args.get(**k).is_none())
        .copied()
        .collect();
    if !missing.is_empty() {
        bail!("missing required arguments: {}", missing.join(", "));
    }
    Ok(())
}
