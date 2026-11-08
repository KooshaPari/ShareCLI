// SPDX-License-Identifier: Apache-2.0 OR MIT
//! MCP manifest loader. Reads `mcp/manifest.json` relative to the crate root
//! (or `$TEAMCOMM_MCP_MANIFEST`) and parses it into a typed structure.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub transport: String,
    pub tools: Vec<ToolDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub returns: serde_json::Value,
}

impl Manifest {
    /// Load the manifest from the crate-relative `mcp/manifest.json`.
    pub fn load_default() -> Result<Self> {
        let path = std::env::var("TEAMCOMM_MCP_MANIFEST")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mcp/manifest.json")
            });
        Self::load(path)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("reading manifest at {}", path.as_ref().display()))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .with_context(|| format!("parsing manifest at {}", path.as_ref().display()))?;
        Ok(manifest)
    }

    /// Look up a tool definition by name (returns None if absent).
    pub fn find_tool(&self, name: &str) -> Option<&ToolDef> {
        self.tools.iter().find(|t| t.name == name)
    }
}
