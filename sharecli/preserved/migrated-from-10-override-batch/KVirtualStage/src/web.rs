// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use tracing::info;

#[cfg(not(feature = "web-ui"))]
use tracing::warn;

#[cfg(feature = "web-ui")]
use axum::{response::Html, routing::get, Router};

#[cfg(feature = "web-ui")]
use tokio::net::TcpListener;

pub struct WebServer {
    // Web server implementation
}

impl WebServer {
    pub async fn new() -> Result<Self> {
        info!("Initializing Web Server");

        Ok(Self {})
    }

    #[cfg(feature = "web-ui")]
    pub async fn start(
        &self,
        host: String,
        port: u16,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        info!("Starting Web server on {}:{}", host, port);

        let app = Router::new()
            .route("/", get(serve_index))
            .route("/dashboard", get(serve_dashboard))
            .route("/sessions", get(serve_sessions))
            .route("/automation", get(serve_automation))
            .route("/recording", get(serve_recording))
            .route("/settings", get(serve_settings));

        let listener = TcpListener::bind(format!("{host}:{port}")).await?;
        info!("Web server listening on {}:{}", host, port);

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
        info!("Web server disabled (web-ui feature not enabled)");
        warn!("Web UI not available. Enable 'web-ui' feature for web interface support.");
        warn!("Host {} and port {} ignored", host, port);

        // Return a handle that immediately completes successfully
        let handle = tokio::spawn(async move {
            info!("Web server not started - web-ui feature disabled");
            Ok(())
        });

        Ok(handle)
    }
}

// Web page handlers
#[cfg(feature = "web-ui")]
async fn serve_index() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>KVirtualStage</title></head>
<body><h1>KVirtualStage Dashboard</h1><p>Web UI not built with full assets</p></body>
</html>"#,
    )
}

#[cfg(feature = "web-ui")]
async fn serve_dashboard() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>Dashboard - KVirtualStage</title></head>
<body><h1>Dashboard</h1><p>Coming soon...</p></body>
</html>"#,
    )
}

#[cfg(feature = "web-ui")]
async fn serve_sessions() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>Sessions - KVirtualStage</title></head>
<body><h1>Sessions</h1><p>Coming soon...</p></body>
</html>"#,
    )
}

#[cfg(feature = "web-ui")]
async fn serve_automation() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>Automation - KVirtualStage</title></head>
<body><h1>Automation</h1><p>Coming soon...</p></body>
</html>"#,
    )
}

#[cfg(feature = "web-ui")]
async fn serve_recording() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>Recording - KVirtualStage</title></head>
<body><h1>Recording</h1><p>Coming soon...</p></body>
</html>"#,
    )
}

#[cfg(feature = "web-ui")]
async fn serve_settings() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html><head><title>Settings - KVirtualStage</title></head>
<body><h1>Settings</h1><p>Coming soon...</p></body>
</html>"#,
    )
}
