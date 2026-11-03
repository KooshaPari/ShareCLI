// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn};

use crate::core::SessionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStorage {
    pub sessions: HashMap<String, SessionInfo>,
    pub last_updated: String,
}

impl SessionStorage {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn get_storage_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home_dir.join(".kvirtualstage").join("sessions.json"))
    }

    pub async fn load() -> Result<Self> {
        let storage_path = Self::get_storage_path()?;

        if !storage_path.exists() {
            info!("No existing session storage found, creating new one");
            let storage = Self::new();
            storage.save().await?;
            return Ok(storage);
        }

        let content = fs::read_to_string(&storage_path).await?;
        let storage: SessionStorage = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse session storage: {}", e))?;

        info!("Loaded {} sessions from storage", storage.sessions.len());
        Ok(storage)
    }

    pub async fn save(&self) -> Result<()> {
        let storage_path = Self::get_storage_path()?;

        // Create directory if it doesn't exist
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&storage_path, content).await?;

        info!("Saved {} sessions to storage", self.sessions.len());
        Ok(())
    }

    pub async fn add_session(&mut self, name: String, session: SessionInfo) -> Result<()> {
        self.sessions.insert(name, session);
        self.last_updated = chrono::Utc::now().to_rfc3339();
        self.save().await?;
        Ok(())
    }

    pub async fn update_session(&mut self, name: &str, session: SessionInfo) -> Result<()> {
        if self.sessions.contains_key(name) {
            self.sessions.insert(name.to_string(), session);
            self.last_updated = chrono::Utc::now().to_rfc3339();
            self.save().await?;
            Ok(())
        } else {
            Err(anyhow!("Session '{}' not found", name))
        }
    }

    pub async fn remove_session(&mut self, name: &str) -> Result<Option<SessionInfo>> {
        let session = self.sessions.remove(name);
        if session.is_some() {
            self.last_updated = chrono::Utc::now().to_rfc3339();
            self.save().await?;
        }
        Ok(session)
    }

    pub fn get_session(&self, name: &str) -> Option<&SessionInfo> {
        self.sessions.get(name)
    }

    pub fn get_session_mut(&mut self, name: &str) -> Option<&mut SessionInfo> {
        self.sessions.get_mut(name)
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.values().cloned().collect()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub async fn cleanup_stale_sessions(&mut self) -> Result<()> {
        let mut stale_sessions: Vec<String> = Vec::new();

        for (name, session) in &self.sessions {
            // Check if container still exists or if session is very old
            if let Some(_container_id) = &session.container_id {
                // Here we would check if container still exists
                // For now, we'll mark sessions older than 24 hours as potentially stale
                if let Ok(created_time) = chrono::DateTime::parse_from_rfc3339(&session.created_at)
                {
                    let now = chrono::Utc::now();
                    let age = now.signed_duration_since(created_time.with_timezone(&chrono::Utc));

                    if age.num_hours() > 24 {
                        warn!("Session '{}' is older than 24 hours, may be stale", name);
                        stale_sessions.push(name.clone());
                    }
                }
            }
        }

        // Remove stale sessions
        for name in stale_sessions {
            warn!("Removing stale session: {}", name);
            self.remove_session(&name).await?;
        }

        Ok(())
    }

    pub async fn backup(&self) -> Result<()> {
        let storage_path = Self::get_storage_path()?;
        let backup_path = storage_path.with_extension("backup.json");

        if storage_path.exists() {
            fs::copy(&storage_path, &backup_path).await?;
            info!("Created backup of session storage");
        }

        Ok(())
    }

    pub async fn restore_from_backup(&mut self) -> Result<()> {
        let storage_path = Self::get_storage_path()?;
        let backup_path = storage_path.with_extension("backup.json");

        if !backup_path.exists() {
            return Err(anyhow!("No backup file found"));
        }

        let content = fs::read_to_string(&backup_path).await?;
        let backup_storage: SessionStorage = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse backup storage: {}", e))?;

        self.sessions = backup_storage.sessions;
        self.last_updated = chrono::Utc::now().to_rfc3339();
        self.save().await?;

        info!("Restored {} sessions from backup", self.sessions.len());
        Ok(())
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
    }
}
