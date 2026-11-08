// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::api::ApiServer;
use crate::audio::AudioManager;
use crate::mcp::McpServer;
use crate::recording::RecordingManager;
use crate::security::SecurityManager;
use crate::ui_automation::UiAutomationEngine;
use crate::virtualization::VirtualizationManager;
use crate::web::WebServer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVirtualStageStatus {
    pub active_sessions: usize,
    pub container_runtime: String,
    pub web_ui_active: bool,
    pub mcp_server_active: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub name: String,
    pub id: String,
    pub desktop: String,
    pub status: String,
    pub created_at: String,
    pub container_id: Option<String>,
    pub vnc_port: Option<u16>,
    pub resources: SessionResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResources {
    pub memory_mb: u64,
    pub cpu_cores: u32,
    pub disk_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVirtualStageConfig {
    pub container_runtime: String,
    pub default_desktop: String,
    pub default_resources: SessionResources,
    pub recording_settings: RecordingSettings,
    pub audio_settings: AudioSettings,
    pub security_settings: SecuritySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSettings {
    pub default_format: String,
    pub quality: String,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub enable_tts: bool,
    pub tts_voice: String,
    pub enable_recording: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub enable_encryption: bool,
    pub credential_vault_path: String,
    pub enable_mfa: bool,
}

pub struct KVirtualStageCore {
    pub sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    config: Arc<RwLock<KVirtualStageConfig>>,
    // Lazy initialization for components requiring external dependencies
    virtualization: Arc<RwLock<Option<VirtualizationManager>>>,
    pub ui_automation: Arc<RwLock<Option<UiAutomationEngine>>>,
    pub recording: Arc<RwLock<Option<RecordingManager>>>,
    pub audio: Arc<RwLock<Option<AudioManager>>>,
    pub security: Arc<RwLock<Option<SecurityManager>>>,
    mcp_server: Arc<RwLock<Option<McpServer>>>,
    api_server: Arc<RwLock<Option<ApiServer>>>,
    web_server: Arc<RwLock<Option<WebServer>>>,
}

impl KVirtualStageCore {
    pub async fn new() -> Result<Self> {
        info!("Initializing KVirtualStage Core");

        let default_config = KVirtualStageConfig {
            container_runtime: "docker".to_string(),
            default_desktop: "kubuntu".to_string(),
            default_resources: SessionResources {
                memory_mb: 2048,
                cpu_cores: 2,
                disk_gb: 10,
            },
            recording_settings: RecordingSettings {
                default_format: "mp4".to_string(),
                quality: "high".to_string(),
                fps: 30,
            },
            audio_settings: AudioSettings {
                enable_tts: true,
                tts_voice: "default".to_string(),
                enable_recording: true,
            },
            security_settings: SecuritySettings {
                enable_encryption: true,
                credential_vault_path: "~/.kvirtualstage/vault".to_string(),
                enable_mfa: false,
            },
        };

        info!("Core initialization completed - components will be lazy-loaded as needed");

        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(default_config)),
            // Initialize as None - will be lazy-loaded when needed
            virtualization: Arc::new(RwLock::new(None)),
            ui_automation: Arc::new(RwLock::new(None)),
            recording: Arc::new(RwLock::new(None)),
            audio: Arc::new(RwLock::new(None)),
            security: Arc::new(RwLock::new(None)),
            mcp_server: Arc::new(RwLock::new(None)),
            api_server: Arc::new(RwLock::new(None)),
            web_server: Arc::new(RwLock::new(None)),
        })
    }

    // Lazy initialization methods for components requiring external dependencies
    async fn ensure_virtualization(&self) -> Result<()> {
        let mut virt_guard = self.virtualization.write().await;
        if virt_guard.is_none() {
            info!("Initializing virtualization manager (Docker required)");
            let vm = VirtualizationManager::new().await?;
            *virt_guard = Some(vm);
        }
        Ok(())
    }

    pub async fn ensure_ui_automation(&self) -> Result<()> {
        let mut ui_guard = self.ui_automation.write().await;
        if ui_guard.is_none() {
            info!("Initializing UI automation engine");
            let ui = UiAutomationEngine::new().await?;
            *ui_guard = Some(ui);
        }
        Ok(())
    }

    async fn ensure_recording(&self) -> Result<()> {
        let mut rec_guard = self.recording.write().await;
        if rec_guard.is_none() {
            info!("Initializing recording manager");
            let rec = RecordingManager::new().await?;
            *rec_guard = Some(rec);
        }
        Ok(())
    }

    pub async fn ensure_audio(&self) -> Result<()> {
        let mut audio_guard = self.audio.write().await;
        if audio_guard.is_none() {
            info!("Initializing audio manager");
            let audio = AudioManager::new().await?;
            *audio_guard = Some(audio);
        }
        Ok(())
    }

    pub async fn ensure_security(&self) -> Result<()> {
        let mut sec_guard = self.security.write().await;
        if sec_guard.is_none() {
            info!("Initializing security manager");
            let sec = SecurityManager::new().await?;
            *sec_guard = Some(sec);
        }
        Ok(())
    }

    pub async fn start_with_ui(&self, host: String, port: u16) -> Result<()> {
        info!("Starting KVirtualStage with Web UI on {}:{}", host, port);

        // Start API server
        let api_server = ApiServer::new(self.clone()).await?;
        let api_handle = api_server.start(host.clone(), port + 1).await?;

        // Start Web server
        let web_server = WebServer::new().await?;
        let web_handle = web_server.start(host, port).await?;

        {
            let mut api_guard = self.api_server.write().await;
            *api_guard = Some(api_server);
        }

        {
            let mut web_guard = self.web_server.write().await;
            *web_guard = Some(web_server);
        }

        // Wait for both servers
        let (api_result, web_result) = tokio::try_join!(api_handle, web_handle)?;
        let _ = api_result;
        let _ = web_result;

        Ok(())
    }

    pub async fn start_headless(&self) -> Result<()> {
        info!("Starting KVirtualStage in headless mode");

        // Start API server only
        let api_server = ApiServer::new(self.clone()).await?;
        let api_handle = api_server.start("127.0.0.1".to_string(), 3001).await?;

        {
            let mut api_guard = self.api_server.write().await;
            *api_guard = Some(api_server);
        }

        let _ = api_handle.await?;

        Ok(())
    }

    pub async fn get_status(&self) -> Result<KVirtualStageStatus> {
        let sessions = self.sessions.read().await;
        let mcp_server = self.mcp_server.read().await;
        let web_server = self.web_server.read().await;

        Ok(KVirtualStageStatus {
            active_sessions: sessions.len(),
            container_runtime: "docker".to_string(),
            web_ui_active: web_server.is_some(),
            mcp_server_active: mcp_server.is_some(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub async fn create_session(
        &self,
        name: String,
        desktop: String,
        image: Option<String>,
        memory: u64,
        cpu: u32,
    ) -> Result<()> {
        info!("Creating session: {}", name);

        // Ensure virtualization manager is initialized (requires Docker)
        self.ensure_virtualization().await?;

        let session_id = Uuid::new_v4().to_string();
        let mut virtualization = self.virtualization.write().await;
        let container_id = virtualization
            .as_mut()
            .ok_or_else(|| anyhow!("Virtualization manager not available"))?
            .create_container(session_id.clone(), desktop.clone(), image, memory, cpu)
            .await?;

        let session_info = SessionInfo {
            name: name.clone(),
            id: session_id,
            desktop,
            status: "created".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            container_id: Some(container_id),
            vnc_port: None,
            resources: SessionResources {
                memory_mb: memory,
                cpu_cores: cpu,
                disk_gb: 10,
            },
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(name, session_info);

        Ok(())
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }

    pub async fn connect_session(&self, name: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(&name) {
            session.status = "connected".to_string();
            info!("Connected to session: {}", name);
            Ok(())
        } else {
            Err(anyhow!("Session '{}' not found", name))
        }
    }

    pub async fn stop_session(&self, name: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(&name) {
            if let Some(container_id) = &session.container_id {
                let virtualization = self.virtualization.read().await;
                if let Some(virt_manager) = virtualization.as_ref() {
                    virt_manager.stop_container(container_id.clone()).await?;
                }
            }
            session.status = "stopped".to_string();
            info!("Stopped session: {}", name);
            Ok(())
        } else {
            Err(anyhow!("Session '{}' not found", name))
        }
    }

    pub async fn remove_session(&self, name: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.remove(&name) {
            if let Some(container_id) = &session.container_id {
                let virtualization = self.virtualization.read().await;
                if let Some(virt_manager) = virtualization.as_ref() {
                    virt_manager.remove_container(container_id.clone()).await?;
                }
            }
            info!("Removed session: {}", name);
            Ok(())
        } else {
            Err(anyhow!("Session '{}' not found", name))
        }
    }

    pub async fn run_script(&self, script: &str) -> Result<()> {
        info!("Running script: {}", script);

        // Load and parse script
        let script_content = tokio::fs::read_to_string(script).await?;
        let ui_automation = self.ui_automation.read().await;
        if let Some(ui_engine) = ui_automation.as_ref() {
            ui_engine.execute_script(script_content).await?;
        }

        Ok(())
    }

    pub async fn run_script_in_session(&self, script: &str, session_name: String) -> Result<()> {
        info!("Running script in session {}: {}", session_name, script);

        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&session_name) {
            let script_content = tokio::fs::read_to_string(script).await?;
            let ui_automation = self.ui_automation.read().await;
            if let Some(ui_engine) = ui_automation.as_ref() {
                ui_engine
                    .execute_script_in_session(script_content, session.id.clone())
                    .await?;
            }
        } else {
            return Err(anyhow!("Session '{}' not found", session_name));
        }

        Ok(())
    }

    pub async fn start_recording(
        &self,
        output: &str,
        format: &str,
        session: Option<String>,
    ) -> Result<()> {
        info!("Starting recording: {} (format: {})", output, format);

        if let Some(session_name) = session {
            let sessions = self.sessions.read().await;
            if let Some(session_info) = sessions.get(&session_name) {
                let mut recording = self.recording.write().await;
                if let Some(recording_manager) = recording.as_mut() {
                    recording_manager
                        .start_recording(
                            output.to_string(),
                            format.to_string(),
                            Some(session_info.id.clone()),
                        )
                        .await?;
                }
            } else {
                return Err(anyhow!("Session '{}' not found", session_name));
            }
        } else {
            let mut recording = self.recording.write().await;
            if let Some(recording_manager) = recording.as_mut() {
                recording_manager
                    .start_recording(output.to_string(), format.to_string(), None)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn take_screenshot(&self, output: &str, session: Option<String>) -> Result<()> {
        info!("Taking screenshot: {}", output);

        if let Some(session_name) = session {
            let sessions = self.sessions.read().await;
            if let Some(session_info) = sessions.get(&session_name) {
                let recording = self.recording.read().await;
                if let Some(recording_manager) = recording.as_ref() {
                    recording_manager
                        .take_screenshot(output.to_string(), Some(session_info.id.clone()))
                        .await?;
                }
            } else {
                return Err(anyhow!("Session '{}' not found", session_name));
            }
        } else {
            let recording = self.recording.read().await;
            if let Some(recording_manager) = recording.as_ref() {
                recording_manager
                    .take_screenshot(output.to_string(), None)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn start_mcp_server(&self, port: u16) -> Result<()> {
        info!("Starting MCP server on port {}", port);

        let mcp_server = McpServer::new(self.clone()).await?;
        let _handle = mcp_server.start(port).await?;

        {
            let mut mcp_guard = self.mcp_server.write().await;
            *mcp_guard = Some(mcp_server);
        }

        Ok(())
    }

    pub async fn stop_mcp_server(&self) -> Result<()> {
        info!("Stopping MCP server");

        let mut mcp_guard = self.mcp_server.write().await;
        if let Some(server) = mcp_guard.take() {
            server.stop().await?;
        }

        Ok(())
    }

    pub async fn list_mcp_tools(&self) -> Result<Vec<McpTool>> {
        let mcp_guard = self.mcp_server.read().await;
        if let Some(server) = mcp_guard.as_ref() {
            server.list_tools().await
        } else {
            // Create a temporary MCP server to get the tools list
            let temp_server = McpServer::new(self.clone()).await?;
            temp_server.list_tools().await
        }
    }

    pub async fn test_mcp_connection(&self, url: String) -> Result<()> {
        info!("Testing MCP connection: {}", url);
        // Implementation for testing MCP connection
        Ok(())
    }

    pub async fn get_config(&self) -> Result<KVirtualStageConfig> {
        let config = self.config.read().await;
        Ok(config.clone())
    }

    pub async fn set_config(&self, key: String, value: String) -> Result<()> {
        info!("Setting config: {} = {}", key, value);

        let mut config = self.config.write().await;

        // Simple key-value configuration update
        match key.as_str() {
            "container_runtime" => config.container_runtime = value,
            "default_desktop" => config.default_desktop = value,
            "recording.default_format" => config.recording_settings.default_format = value,
            "audio.tts_voice" => config.audio_settings.tts_voice = value,
            _ => return Err(anyhow!("Unknown configuration key: {}", key)),
        }

        Ok(())
    }

    pub async fn init_config(&self) -> Result<()> {
        info!("Initializing configuration");

        // Create config directory
        let config_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not find home directory"))?
            .join(".kvirtualstage");

        tokio::fs::create_dir_all(&config_dir).await?;

        // Save default config
        let config = self.config.read().await;
        let config_path = config_dir.join("config.toml");
        let config_content = toml::to_string(&*config)?;
        tokio::fs::write(config_path, config_content).await?;

        Ok(())
    }
}

// Clone implementation for API server
impl Clone for KVirtualStageCore {
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            config: Arc::clone(&self.config),
            virtualization: Arc::clone(&self.virtualization),
            ui_automation: Arc::clone(&self.ui_automation),
            recording: Arc::clone(&self.recording),
            audio: Arc::clone(&self.audio),
            security: Arc::clone(&self.security),
            mcp_server: Arc::clone(&self.mcp_server),
            api_server: Arc::clone(&self.api_server),
            web_server: Arc::clone(&self.web_server),
        }
    }
}
