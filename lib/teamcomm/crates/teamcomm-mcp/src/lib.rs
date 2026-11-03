// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `teamcomm-mcp` — MCP (Model Context Protocol) stdio server that bridges
//! MCP tool calls to the teamcomm daemon via the `teamcomm-client` crate.
//!
//! See `mcp/manifest.json` for the full tool catalog and argument shapes.
//! All bridges are thin: they forward MCP tool arguments (under friendly
//! field names like `session_id`, `path`, `mode`) to the typed
//! `teamcomm-client` methods, which speak the daemon's JSON-RPC over Unix
//! socket. No new wire protocol is invented on the MCP side.

pub mod dispatch;
pub mod handlers;
pub mod manifest;

pub use dispatch::{route, dispatch_tool_call, DispatchError};
pub use handlers::{ToolContext, KNOWN_TOOL_NAMES, ToolResult};
pub use manifest::{Manifest, ToolDef};
