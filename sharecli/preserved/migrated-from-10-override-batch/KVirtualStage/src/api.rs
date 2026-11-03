// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[cfg(not(feature = "web-ui"))]
use tracing::warn;

#[cfg(feature = "web-ui")]
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{delete, get, post, put},
    Router,
};

#[cfg(feature = "web-ui")]
use tokio::net::TcpListener;

#[cfg(feature = "web-ui")]
use tower_http::cors::CorsLayer;

use crate::core::KVirtualStageCore;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    pub desktop: Option<String>,
    pub image: Option<String>,
    pub memory: Option<u64>,
    pub cpu: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct RunScriptRequest {
    pub script: String,
    pub session: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordingRequest {
    pub output: String,
    pub format: Option<String>,
    pub session: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScreenshotRequest {
    pub output: String,
    pub session: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UiActionRequest {
    pub action_type: String,
    pub target: Option<String>,
    pub coordinates: Option<(i32, i32)>,
    pub text: Option<String>,
    pub session: Option<String>,
}

#[derive(Clone)]
pub struct ApiServer {
    core: KVirtualStageCore,
}

impl ApiServer {
    pub async fn new(core: KVirtualStageCore) -> Result<Self> {
        info!("Initializing API Server");

        Ok(Self { core })
    }

    #[cfg(feature = "web-ui")]
    pub async fn start(
        &self,
        host: String,
        port: u16,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        info!(
            "Starting API server with HTTP interface on {}:{}",
            host, port
        );

        let app = Router::new()
            // System routes
            .route("/api/status", get(get_status))
            .route("/api/config", get(get_config))
            .route("/api/config", put(update_config))
            // Session routes
            .route("/api/sessions", get(list_sessions))
            .route("/api/sessions", post(create_session))
            .route("/api/sessions/:name", get(get_session))
            .route("/api/sessions/:name", delete(remove_session))
            .route("/api/sessions/:name/start", post(start_session))
            .route("/api/sessions/:name/stop", post(stop_session))
            .route("/api/sessions/:name/connect", post(connect_session))
            // Automation routes
            .route("/api/automation/run", post(run_automation))
            .route("/api/automation/ui-action", post(execute_ui_action))
            // Recording routes
            .route("/api/recording/start", post(start_recording))
            .route("/api/recording/stop/:id", post(stop_recording))
            .route("/api/recording/screenshot", post(take_screenshot))
            // Audio routes
            .route("/api/audio/devices", get(list_audio_devices))
            .route("/api/audio/tts", post(text_to_speech))
            .route("/api/audio/record/start", post(start_audio_recording))
            .route("/api/audio/record/stop/:id", post(stop_audio_recording))
            // Security routes
            .route("/api/security/credentials", get(list_credentials))
            .route("/api/security/credentials", post(store_credential))
            .route("/api/security/credentials/:id", get(get_credential))
            .route("/api/security/credentials/:id", delete(remove_credential))
            // MCP routes
            .route("/api/mcp/tools", get(list_mcp_tools))
            .route("/api/mcp/start", post(start_mcp_server))
            .route("/api/mcp/stop", post(stop_mcp_server))
            .layer(CorsLayer::permissive())
            .with_state(self.clone());

        let listener = TcpListener::bind(format!("{}:{}", host, port)).await?;
        info!("API server listening on {}:{}", host, port);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| anyhow!("Server error: {}", e))
        });

        Ok(handle)
    }

    #[cfg(not(feature = "web-ui"))]
    pub async fn start(
        &self,
        host: String,
        port: u16,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        info!("API server disabled (web-ui feature not enabled)");
        warn!("HTTP API server not available. Enable 'web-ui' feature for HTTP API support.");
        warn!("Host {} and port {} ignored", host, port);

        // Return a handle that immediately completes successfully
        let handle = tokio::spawn(async move {
            info!("API server not started - web-ui feature disabled");
            Ok(())
        });

        Ok(handle)
    }
}

#[cfg(feature = "web-ui")]
fn success_response<T: Serialize>(data: T) -> ResponseJson<ApiResponse<T>> {
    ResponseJson(ApiResponse {
        success: true,
        data: Some(data),
        error: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

#[cfg(feature = "web-ui")]
fn error_response<T>(error: String) -> ResponseJson<ApiResponse<T>> {
    ResponseJson(ApiResponse {
        success: false,
        data: None,
        error: Some(error),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

// HTTP API handlers (require web-ui feature)
#[cfg(feature = "web-ui")]
async fn get_status(
    State(server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<crate::core::KVirtualStageStatus>>, StatusCode> {
    match server.core.get_status().await {
        Ok(status) => Ok(success_response(status)),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn get_config(
    State(server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<crate::core::KVirtualStageConfig>>, StatusCode> {
    match server.core.get_config().await {
        Ok(config) => Ok(success_response(config)),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn update_config(
    State(server): State<ApiServer>,
    Json(config): Json<HashMap<String, String>>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    for (key, value) in config {
        if let Err(e) = server.core.set_config(key, value).await {
            return Ok(error_response(e.to_string()));
        }
    }
    Ok(success_response(()))
}

// Session handlers
#[cfg(feature = "web-ui")]
async fn list_sessions(
    State(server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<Vec<crate::core::SessionInfo>>>, StatusCode> {
    match server.core.list_sessions().await {
        Ok(sessions) => Ok(success_response(sessions)),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn create_session(
    State(server): State<ApiServer>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    let desktop = request.desktop.unwrap_or_else(|| "kubuntu".to_string());
    let memory = request.memory.unwrap_or(2048);
    let cpu = request.cpu.unwrap_or(2);

    match server
        .core
        .create_session(request.name, desktop, request.image, memory, cpu)
        .await
    {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn get_session(
    State(server): State<ApiServer>,
    Path(name): Path<String>,
) -> Result<ResponseJson<ApiResponse<Option<crate::core::SessionInfo>>>, StatusCode> {
    match server.core.list_sessions().await {
        Ok(sessions) => {
            let session = sessions.into_iter().find(|s| s.name == name);
            Ok(success_response(session))
        }
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn remove_session(
    State(server): State<ApiServer>,
    Path(name): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.remove_session(name).await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn start_session(
    State(server): State<ApiServer>,
    Path(name): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.connect_session(name).await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn stop_session(
    State(server): State<ApiServer>,
    Path(name): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.stop_session(name).await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn connect_session(
    State(server): State<ApiServer>,
    Path(name): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.connect_session(name).await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

// Automation handlers
#[cfg(feature = "web-ui")]
async fn run_automation(
    State(server): State<ApiServer>,
    Json(request): Json<RunScriptRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    let result = if let Some(session) = request.session {
        server
            .core
            .run_script_in_session(&request.script, session)
            .await
    } else {
        server.core.run_script(&request.script).await
    };

    match result {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn execute_ui_action(
    State(_server): State<ApiServer>,
    Json(_request): Json<UiActionRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for UI actions
    Ok(success_response(()))
}

// Recording handlers
#[cfg(feature = "web-ui")]
async fn start_recording(
    State(server): State<ApiServer>,
    Json(request): Json<RecordingRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    let format = request.format.unwrap_or_else(|| "mp4".to_string());

    match server
        .core
        .start_recording(&request.output, &format, request.session)
        .await
    {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn stop_recording(
    State(_server): State<ApiServer>,
    Path(_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for stopping recording
    Ok(success_response(()))
}

#[cfg(feature = "web-ui")]
async fn take_screenshot(
    State(server): State<ApiServer>,
    Json(request): Json<ScreenshotRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server
        .core
        .take_screenshot(&request.output, request.session)
        .await
    {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

// Audio handlers
#[cfg(feature = "web-ui")]
async fn list_audio_devices(
    State(_server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<Vec<crate::audio::AudioDevice>>>, StatusCode> {
    // Implementation for listing audio devices
    Ok(success_response(vec![]))
}

#[cfg(feature = "web-ui")]
async fn text_to_speech(
    State(_server): State<ApiServer>,
    Json(_request): Json<crate::audio::TtsRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for text to speech
    Ok(success_response(()))
}

#[cfg(feature = "web-ui")]
async fn start_audio_recording(
    State(_server): State<ApiServer>,
    Json(_request): Json<RecordingRequest>,
) -> Result<ResponseJson<ApiResponse<String>>, StatusCode> {
    // Implementation for starting audio recording
    Ok(success_response("recording_id".to_string()))
}

#[cfg(feature = "web-ui")]
async fn stop_audio_recording(
    State(_server): State<ApiServer>,
    Path(_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for stopping audio recording
    Ok(success_response(()))
}

// Security handlers
#[cfg(feature = "web-ui")]
async fn list_credentials(
    State(_server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<Vec<crate::security::Credential>>>, StatusCode> {
    // Implementation for listing credentials
    Ok(success_response(vec![]))
}

#[cfg(feature = "web-ui")]
async fn store_credential(
    State(_server): State<ApiServer>,
    Json(_credential): Json<crate::security::Credential>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for storing credential
    Ok(success_response(()))
}

#[cfg(feature = "web-ui")]
async fn get_credential(
    State(_server): State<ApiServer>,
    Path(_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<Option<crate::security::Credential>>>, StatusCode> {
    // Implementation for getting credential
    Ok(success_response(None))
}

#[cfg(feature = "web-ui")]
async fn remove_credential(
    State(_server): State<ApiServer>,
    Path(_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    // Implementation for removing credential
    Ok(success_response(()))
}

// MCP handlers
#[cfg(feature = "web-ui")]
async fn list_mcp_tools(
    State(server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<Vec<crate::core::McpTool>>>, StatusCode> {
    match server.core.list_mcp_tools().await {
        Ok(tools) => Ok(success_response(tools)),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn start_mcp_server(
    State(server): State<ApiServer>,
    Json(port): Json<u16>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.start_mcp_server(port).await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}

#[cfg(feature = "web-ui")]
async fn stop_mcp_server(
    State(server): State<ApiServer>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match server.core.stop_mcp_server().await {
        Ok(_) => Ok(success_response(())),
        Err(e) => Ok(error_response(e.to_string())),
    }
}
