// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Plugin registry: discover and call executable plugins from a directory.
//!
//! Wired into the binary via `util` demos / tests; keep the surface under
//! `#[allow(dead_code)]` until a first-class `sharecli plugin` subcommand lands.
#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    pub plugins: Vec<PluginManifest>,
}

impl PluginRegistry {
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut plugins = Vec::new();
        if !dir.exists() {
            return Ok(Self { plugins });
        }
        for e in std::fs::read_dir(dir)?.flatten() {
            let p = e.path();
            if p.is_file() {
                plugins.push(PluginManifest {
                    name: p.file_stem().unwrap_or_default().to_string_lossy().to_string(),
                    path: p,
                });
            }
        }
        Ok(Self { plugins })
    }
    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.iter().find(|p| p.name == name)
    }
    pub fn call(&self, name: &str, input: &str) -> Result<String> {
        let pl = self.get(name).ok_or_else(|| anyhow::anyhow!("plugin '{}' not found", name))?;
        if !pl.path.exists() {
            bail!("plugin binary not found: {}", pl.path.display());
        }
        Ok(String::from_utf8_lossy(&Command::new(&pl.path).arg(input).output()?.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_dir() {
        let d = tempfile::TempDir::new().unwrap();
        assert!(PluginRegistry::load_dir(d.path()).unwrap().plugins.is_empty());
    }
    #[test]
    fn nonexistent_dir() {
        assert!(PluginRegistry::load_dir(Path::new("/no/such/dir")).unwrap().plugins.is_empty());
    }
    #[test]
    fn finds_files() {
        let d = tempfile::TempDir::new().unwrap();
        std::fs::write(d.path().join("my-plugin"), "#!/bin/sh\necho hi").unwrap();
        let r = PluginRegistry::load_dir(d.path()).unwrap();
        assert_eq!(r.plugins.len(), 1);
        assert_eq!(r.plugins[0].name, "my-plugin");
    }
    #[test]
    fn get_missing() {
        assert!(PluginRegistry::default().get("x").is_none());
    }
    #[test]
    fn call_missing() {
        assert!(PluginRegistry::default().call("x", "").is_err());
    }
    #[test]
    fn call_ghost_binary() {
        let d = tempfile::TempDir::new().unwrap();
        let r = PluginRegistry {
            plugins: vec![PluginManifest { name: "x".into(), path: d.path().join("ghost") }],
        };
        assert!(r.call("x", "").is_err());
    }
}
