// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[cfg(not(feature = "web-ui"))]
use tracing::warn;

#[cfg(feature = "web-ui")]
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};

#[cfg(feature = "web-ui")]
use tokio::net::TcpListener;

#[cfg(feature = "web-ui")]
use tower_http::cors::CorsLayer;

use crate::core::{KVirtualStageCore, McpTool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub method: String,
    pub params: serde_json::Value,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub success: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct McpServer {
    core: KVirtualStageCore,
    tools: HashMap<String, McpTool>,
}

impl McpServer {
    pub async fn new(core: KVirtualStageCore) -> Result<Self> {
        info!("Initializing MCP Server");

        let mut server = Self {
            core,
            tools: HashMap::new(),
        };

        // Register MCP tools
        server.register_tools().await?;

        Ok(server)
    }

    #[cfg(feature = "web-ui")]
    pub async fn start(&self, port: u16) -> Result<tokio::task::JoinHandle<Result<()>>> {
        info!("Starting MCP server with HTTP interface on port {}", port);

        let app = Router::new()
            .route("/mcp", post(handle_mcp_request))
            .route("/mcp/tools", get(list_tools))
            .route("/mcp/health", get(health_check))
            .layer(CorsLayer::permissive())
            .with_state(self.clone());

        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("MCP server listening on port {}", port);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| anyhow!("Server error: {}", e))
        });

        Ok(handle)
    }

    #[cfg(not(feature = "web-ui"))]
    pub async fn start(&self, port: u16) -> Result<tokio::task::JoinHandle<Result<()>>> {
        info!(
            "Starting MCP server in stdio mode (HTTP server disabled - web-ui feature not enabled)"
        );
        warn!("HTTP MCP server not available. Enable 'web-ui' feature for HTTP support.");
        warn!("Port {} ignored in stdio mode", port);

        // Return a handle that immediately completes successfully
        let handle = tokio::spawn(async move {
            info!("MCP server running in stdio mode");
            // Keep the task alive but don't bind to any port
            tokio::signal::ctrl_c()
                .await
                .map_err(|e| anyhow!("Signal error: {}", e))?;
            info!("MCP server shutting down");
            Ok(())
        });

        Ok(handle)
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping MCP server");
        // Implementation for graceful shutdown
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        Ok(self.tools.values().cloned().collect())
    }

    pub async fn execute_tool(&self, tool_call: McpToolCall) -> Result<McpToolResult> {
        info!("Executing MCP tool: {}", tool_call.name);

        match tool_call.name.as_str() {
            "create_session" => self.tool_create_session(tool_call.arguments).await,
            "run_automation" => self.tool_run_automation(tool_call.arguments).await,
            "take_screenshot" => self.tool_take_screenshot(tool_call.arguments).await,
            "record_screen" => self.tool_record_screen(tool_call.arguments).await,
            "click_element" => self.tool_click_element(tool_call.arguments).await,
            "type_text" => self.tool_type_text(tool_call.arguments).await,
            "find_element" => self.tool_find_element(tool_call.arguments).await,
            "get_sessions" => self.tool_get_sessions(tool_call.arguments).await,
            "text_to_speech" => self.tool_text_to_speech(tool_call.arguments).await,
            "get_credentials" => self.tool_get_credentials(tool_call.arguments).await,
            _ => Err(anyhow!("Unknown tool: {}", tool_call.name)),
        }
    }

    async fn register_tools(&mut self) -> Result<()> {
        info!("Registering MCP tools");

        // Session management tools
        self.tools.insert("create_session".to_string(), McpTool {
            name: "create_session".to_string(),
            description: "Create a new desktop automation session".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Session name"},
                    "desktop": {"type": "string", "description": "Desktop environment (kubuntu, ubuntu, debian)"},
                    "memory": {"type": "integer", "description": "Memory in MB"},
                    "cpu": {"type": "integer", "description": "CPU cores"}
                },
                "required": ["name"]
            }),
        });

        // Automation tools
        self.tools.insert("run_automation".to_string(), McpTool {
            name: "run_automation".to_string(),
            description: "Run an automation script".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "script": {"type": "string", "description": "Automation script content or path"},
                    "session": {"type": "string", "description": "Session name (optional)"}
                },
                "required": ["script"]
            }),
        });

        // Recording tools
        self.tools.insert(
            "take_screenshot".to_string(),
            McpTool {
                name: "take_screenshot".to_string(),
                description: "Take a screenshot of the desktop".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "output": {"type": "string", "description": "Output file path"},
                        "session": {"type": "string", "description": "Session name (optional)"}
                    },
                    "required": ["output"]
                }),
            },
        );

        self.tools.insert("record_screen".to_string(), McpTool {
            name: "record_screen".to_string(),
            description: "Start screen recording".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "output": {"type": "string", "description": "Output file path"},
                    "format": {"type": "string", "description": "Recording format (mp4, gif, webm)"},
                    "session": {"type": "string", "description": "Session name (optional)"}
                },
                "required": ["output"]
            }),
        });

        // UI automation tools
        self.tools.insert(
            "click_element".to_string(),
            McpTool {
                name: "click_element".to_string(),
                description: "Click on a UI element".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "x": {"type": "integer", "description": "X coordinate"},
                        "y": {"type": "integer", "description": "Y coordinate"},
                        "session": {"type": "string", "description": "Session name (optional)"}
                    },
                    "required": ["x", "y"]
                }),
            },
        );

        self.tools.insert(
            "type_text".to_string(),
            McpTool {
                name: "type_text".to_string(),
                description: "Type text into the focused element".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "Text to type"},
                        "session": {"type": "string", "description": "Session name (optional)"}
                    },
                    "required": ["text"]
                }),
            },
        );

        self.tools.insert("find_element".to_string(), McpTool {
            name: "find_element".to_string(),
            description: "Find UI elements on the screen".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string", "description": "Element selector or description"},
                    "session": {"type": "string", "description": "Session name (optional)"}
                },
                "required": ["selector"]
            }),
        });

        // Information tools
        self.tools.insert(
            "get_sessions".to_string(),
            McpTool {
                name: "get_sessions".to_string(),
                description: "Get list of active sessions".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        );

        // Audio tools
        self.tools.insert(
            "text_to_speech".to_string(),
            McpTool {
                name: "text_to_speech".to_string(),
                description: "Convert text to speech".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "Text to convert"},
                        "voice": {"type": "string", "description": "Voice to use (optional)"},
                        "session": {"type": "string", "description": "Session name (optional)"}
                    },
                    "required": ["text"]
                }),
            },
        );

        // Security tools
        self.tools.insert(
            "get_credentials".to_string(),
            McpTool {
                name: "get_credentials".to_string(),
                description: "Get stored credentials for a service".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "service": {"type": "string", "description": "Service name"},
                        "username": {"type": "string", "description": "Username (optional)"}
                    },
                    "required": ["service"]
                }),
            },
        );

        info!("Registered {} MCP tools", self.tools.len());
        Ok(())
    }

    async fn tool_create_session(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: name"))?;

        let desktop = args
            .get("desktop")
            .and_then(|v| v.as_str())
            .unwrap_or("kubuntu");

        let memory = args.get("memory").and_then(|v| v.as_u64()).unwrap_or(2048);

        let cpu = args.get("cpu").and_then(|v| v.as_u64()).unwrap_or(2) as u32;

        match self
            .core
            .create_session(name.to_string(), desktop.to_string(), None, memory, cpu)
            .await
        {
            Ok(_) => Ok(McpToolResult {
                success: true,
                result: serde_json::json!({
                    "session_name": name,
                    "desktop": desktop,
                    "memory": memory,
                    "cpu": cpu
                }),
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn tool_run_automation(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let script = args
            .get("script")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: script"))?;

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let result = if let Some(session_name) = session {
            self.core.run_script_in_session(script, session_name).await
        } else {
            self.core.run_script(script).await
        };

        match result {
            Ok(_) => Ok(McpToolResult {
                success: true,
                result: serde_json::json!({"message": "Automation executed successfully"}),
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn tool_take_screenshot(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let output = args
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: output"))?;

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        match self.core.take_screenshot(output, session).await {
            Ok(_) => Ok(McpToolResult {
                success: true,
                result: serde_json::json!({"screenshot_path": output}),
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn tool_record_screen(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let output = args
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: output"))?;

        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("mp4");

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        match self.core.start_recording(output, format, session).await {
            Ok(_) => Ok(McpToolResult {
                success: true,
                result: serde_json::json!({
                    "recording_path": output,
                    "format": format
                }),
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn tool_click_element(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let x = args
            .get("x")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("Missing required parameter: x"))? as i32;

        let y = args
            .get("y")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("Missing required parameter: y"))? as i32;

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Ensure UI automation is initialized
        if let Err(e) = self.core.ensure_ui_automation().await {
            return Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(format!("Failed to initialize UI automation: {}", e)),
            });
        }

        // Create a click action
        let click_action = crate::ui_automation::UiAction {
            action_type: "click".to_string(),
            target: None,
            coordinates: Some((x, y)),
            text: None,
            delay: None,
        };

        let ui_automation = self.core.ui_automation.read().await;
        let session_id = if let Some(session_name) = session {
            let sessions = self.core.sessions.read().await;
            sessions.get(&session_name).map(|s| s.id.clone())
        } else {
            None
        };

        match ui_automation.as_ref() {
            Some(ui) => match ui.execute_action(&click_action, session_id).await {
                Ok(_) => Ok(McpToolResult {
                    success: true,
                    result: serde_json::json!({
                        "message": "Element clicked successfully",
                        "coordinates": {"x": x, "y": y}
                    }),
                    error: None,
                }),
                Err(e) => Ok(McpToolResult {
                    success: false,
                    result: serde_json::json!(null),
                    error: Some(e.to_string()),
                }),
            },
            None => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some("UI automation not initialized".to_string()),
            }),
        }
    }

    async fn tool_type_text(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: text"))?;

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Ensure UI automation is initialized
        if let Err(e) = self.core.ensure_ui_automation().await {
            return Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(format!("Failed to initialize UI automation: {}", e)),
            });
        }

        // Create a type action
        let type_action = crate::ui_automation::UiAction {
            action_type: "type".to_string(),
            target: None,
            coordinates: None,
            text: Some(text.to_string()),
            delay: None,
        };

        let ui_automation = self.core.ui_automation.read().await;
        let session_id = if let Some(session_name) = session {
            let sessions = self.core.sessions.read().await;
            sessions.get(&session_name).map(|s| s.id.clone())
        } else {
            None
        };

        match ui_automation.as_ref() {
            Some(ui) => match ui.execute_action(&type_action, session_id).await {
                Ok(_) => Ok(McpToolResult {
                    success: true,
                    result: serde_json::json!({
                        "message": "Text typed successfully",
                        "text": text
                    }),
                    error: None,
                }),
                Err(e) => Ok(McpToolResult {
                    success: false,
                    result: serde_json::json!(null),
                    error: Some(e.to_string()),
                }),
            },
            None => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some("UI automation not initialized".to_string()),
            }),
        }
    }

    async fn tool_find_element(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let selector = args
            .get("selector")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: selector"))?;

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Ensure UI automation is initialized
        if let Err(e) = self.core.ensure_ui_automation().await {
            return Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(format!("Failed to initialize UI automation: {}", e)),
            });
        }

        let ui_automation = self.core.ui_automation.read().await;
        let session_id = if let Some(session_name) = session {
            let sessions = self.core.sessions.read().await;
            sessions.get(&session_name).map(|s| s.id.clone())
        } else {
            None
        };

        match ui_automation.as_ref() {
            Some(ui) => match ui.find_elements(session_id, selector.to_string()).await {
                Ok(elements) => Ok(McpToolResult {
                    success: true,
                    result: serde_json::json!({
                        "elements": elements,
                        "selector": selector,
                        "count": elements.len()
                    }),
                    error: None,
                }),
                Err(e) => Ok(McpToolResult {
                    success: false,
                    result: serde_json::json!(null),
                    error: Some(e.to_string()),
                }),
            },
            None => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some("UI automation not initialized".to_string()),
            }),
        }
    }

    async fn tool_get_sessions(
        &self,
        _args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        match self.core.list_sessions().await {
            Ok(sessions) => Ok(McpToolResult {
                success: true,
                result: serde_json::json!({"sessions": sessions}),
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn tool_text_to_speech(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: text"))?;

        let voice = args
            .get("voice")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let session = args
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Ensure audio manager is initialized
        if let Err(e) = self.core.ensure_audio().await {
            return Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(format!("Failed to initialize audio manager: {}", e)),
            });
        }

        let audio_manager = self.core.audio.read().await;

        match audio_manager.as_ref() {
            Some(audio) => {
                let tts_request = crate::audio::TtsRequest {
                    text: text.to_string(),
                    voice: voice.to_string(),
                    speed: 1.0,
                    pitch: 1.0,
                };

                match audio.text_to_speech(tts_request).await {
                    Ok(audio_data) => {
                        // Optionally play the audio to virtual microphone
                        match audio.play_audio_to_virtual_mic(audio_data).await {
                            Ok(_) => Ok(McpToolResult {
                                success: true,
                                result: serde_json::json!({
                                    "message": "Text converted to speech and played",
                                    "text": text,
                                    "voice": voice,
                                    "session": session
                                }),
                                error: None,
                            }),
                            Err(e) => Ok(McpToolResult {
                                success: false,
                                result: serde_json::json!(null),
                                error: Some(format!("Failed to play audio: {}", e)),
                            }),
                        }
                    }
                    Err(e) => Ok(McpToolResult {
                        success: false,
                        result: serde_json::json!(null),
                        error: Some(e.to_string()),
                    }),
                }
            }
            None => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some("Audio manager not initialized".to_string()),
            }),
        }
    }

    async fn tool_get_credentials(
        &self,
        args: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult> {
        let service = args
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let username = args
            .get("username")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Ensure security manager is initialized
        if let Err(e) = self.core.ensure_security().await {
            return Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some(format!("Failed to initialize security manager: {}", e)),
            });
        }

        let security_manager = self.core.security.read().await;

        match security_manager.as_ref() {
            Some(security) => {
                // First try to get OAuth token
                match security.get_oauth_token(service.to_string()).await {
                    Ok(oauth_token) => {
                        // Return OAuth token with sensitive data redacted
                        Ok(McpToolResult {
                            success: true,
                            result: serde_json::json!({
                                "service": service,
                                "credential_type": "oauth",
                                "token_type": oauth_token.token_type,
                                "expires_in": oauth_token.expires_in,
                                "scope": oauth_token.scope,
                                "has_access_token": !oauth_token.access_token.is_empty(),
                                "has_refresh_token": oauth_token.refresh_token.is_some(),
                                // Don't expose actual tokens for security
                                "access_token": "[REDACTED]",
                                "refresh_token": if oauth_token.refresh_token.is_some() {
                                    serde_json::Value::String("[REDACTED]".to_string())
                                } else {
                                    serde_json::Value::Null
                                }
                            }),
                            error: None,
                        })
                    }
                    Err(_) => {
                        // Try to get password if OAuth token not found
                        if let Some(username) = username {
                            match security
                                .get_password(service.to_string(), username.clone())
                                .await
                            {
                                Ok(_password) => {
                                    // Return password credential info with sensitive data redacted
                                    Ok(McpToolResult {
                                        success: true,
                                        result: serde_json::json!({
                                            "service": service,
                                            "credential_type": "password",
                                            "username": username,
                                            "has_password": true,
                                            "password": "[REDACTED]"
                                        }),
                                        error: None,
                                    })
                                }
                                Err(e) => Ok(McpToolResult {
                                    success: false,
                                    result: serde_json::json!(null),
                                    error: Some(format!(
                                        "No credentials found for service '{}': {}",
                                        service, e
                                    )),
                                }),
                            }
                        } else {
                            // List available credentials for the service
                            match security.list_credentials().await {
                                Ok(credentials) => {
                                    let service_credentials: Vec<_> = credentials
                                        .into_iter()
                                        .filter(|c| c.service == service)
                                        .map(|c| {
                                            serde_json::json!({
                                                "id": c.id,
                                                "name": c.name,
                                                "credential_type": c.credential_type,
                                                "username": c.username,
                                                "created_at": c.created_at,
                                                "expires_at": c.expires_at
                                            })
                                        })
                                        .collect();

                                    Ok(McpToolResult {
                                        success: true,
                                        result: serde_json::json!({
                                            "service": service,
                                            "credentials": service_credentials,
                                            "count": service_credentials.len()
                                        }),
                                        error: None,
                                    })
                                }
                                Err(e) => Ok(McpToolResult {
                                    success: false,
                                    result: serde_json::json!(null),
                                    error: Some(e.to_string()),
                                }),
                            }
                        }
                    }
                }
            }
            None => Ok(McpToolResult {
                success: false,
                result: serde_json::json!(null),
                error: Some("Security manager not initialized".to_string()),
            }),
        }
    }
}

// HTTP handlers
#[cfg(feature = "web-ui")]
async fn handle_mcp_request(
    State(server): State<McpServer>,
    Json(request): Json<McpRequest>,
) -> Result<ResponseJson<McpResponse>, StatusCode> {
    info!("Handling MCP request: {}", request.method);

    match request.method.as_str() {
        "tools/call" => {
            let tool_call: McpToolCall =
                serde_json::from_value(request.params).map_err(|_| StatusCode::BAD_REQUEST)?;

            match server.execute_tool(tool_call).await {
                Ok(result) => Ok(ResponseJson(McpResponse {
                    result: Some(serde_json::to_value(result).unwrap()),
                    error: None,
                    id: request.id,
                })),
                Err(e) => Ok(ResponseJson(McpResponse {
                    result: None,
                    error: Some(McpError {
                        code: -32603,
                        message: e.to_string(),
                        data: None,
                    }),
                    id: request.id,
                })),
            }
        }
        "tools/list" => match server.list_tools().await {
            Ok(tools) => Ok(ResponseJson(McpResponse {
                result: Some(serde_json::to_value(tools).unwrap()),
                error: None,
                id: request.id,
            })),
            Err(e) => Ok(ResponseJson(McpResponse {
                result: None,
                error: Some(McpError {
                    code: -32603,
                    message: e.to_string(),
                    data: None,
                }),
                id: request.id,
            })),
        },
        _ => Ok(ResponseJson(McpResponse {
            result: None,
            error: Some(McpError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
            id: request.id,
        })),
    }
}

#[cfg(feature = "web-ui")]
async fn list_tools(
    State(server): State<McpServer>,
) -> Result<ResponseJson<Vec<McpTool>>, StatusCode> {
    match server.list_tools().await {
        Ok(tools) => Ok(ResponseJson(tools)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[cfg(feature = "web-ui")]
async fn health_check() -> ResponseJson<serde_json::Value> {
    ResponseJson(serde_json::json!({
        "status": "healthy",
        "service": "kvirtualstage-mcp",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
