// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    pub id: String,
    pub session_id: Option<String>,
    pub output_path: String,
    pub format: String,
    pub status: String,
    pub started_at: String,
    pub duration: Option<u64>,
}

pub struct RecordingManager {
    active_recordings: HashMap<String, RecordingSession>,
    ffmpeg_path: PathBuf,
}

impl RecordingManager {
    pub async fn new() -> Result<Self> {
        info!("Initializing Recording Manager");

        // Check if ffmpeg is available
        let ffmpeg_path = Self::find_ffmpeg().await?;

        Ok(Self {
            active_recordings: HashMap::new(),
            ffmpeg_path,
        })
    }

    pub async fn start_recording(
        &mut self,
        output_path: String,
        format: String,
        session_id: Option<String>,
    ) -> Result<String> {
        info!("Starting recording: {} (format: {})", output_path, format);

        let recording_id = uuid::Uuid::new_v4().to_string();

        // Determine display and input source
        let display = if let Some(ref _session_id) = session_id {
            format!(":0") // Container display
        } else {
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string())
        };

        // Start recording based on format
        match format.as_str() {
            "mp4" => {
                self.start_mp4_recording(&recording_id, &output_path, &display)
                    .await?
            }
            "webm" => {
                self.start_webm_recording(&recording_id, &output_path, &display)
                    .await?
            }
            "gif" => {
                self.start_gif_recording(&recording_id, &output_path, &display)
                    .await?
            }
            _ => return Err(anyhow!("Unsupported recording format: {}", format)),
        }

        let recording_session = RecordingSession {
            id: recording_id.clone(),
            session_id,
            output_path,
            format,
            status: "recording".to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            duration: None,
        };

        self.active_recordings
            .insert(recording_id.clone(), recording_session);

        Ok(recording_id)
    }

    pub async fn stop_recording(&mut self, recording_id: String) -> Result<()> {
        info!("Stopping recording: {}", recording_id);

        if let Some(recording) = self.active_recordings.get_mut(&recording_id) {
            recording.status = "stopped".to_string();

            // Stop the ffmpeg process
            // This is a simplified implementation
            // In a real implementation, we would track the process and send SIGTERM

            Ok(())
        } else {
            Err(anyhow!("Recording not found: {}", recording_id))
        }
    }

    pub async fn take_screenshot(
        &self,
        output_path: String,
        session_id: Option<String>,
    ) -> Result<()> {
        info!("Taking screenshot: {}", output_path);

        let display = if let Some(_session_id) = session_id {
            ":0".to_string() // Container display
        } else {
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string())
        };

        // Use ImageMagick's import command
        let output = Command::new("import")
            .env("DISPLAY", display)
            .args(["-window", "root", &output_path])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to take screenshot: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("Screenshot saved: {}", output_path);
        Ok(())
    }

    pub async fn convert_to_gif(&self, input_path: String, output_path: String) -> Result<()> {
        info!("Converting to GIF: {} -> {}", input_path, output_path);

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args([
            "-i",
            &input_path,
            "-vf",
            "fps=10,scale=1024:-1:flags=lanczos,palettegen",
            "-y",
            "palette.png",
        ]);

        let output = cmd.output().await?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to generate palette: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args([
            "-i",
            &input_path,
            "-i",
            "palette.png",
            "-filter_complex",
            "fps=10,scale=1024:-1:flags=lanczos[x];[x][1:v]paletteuse",
            "-y",
            &output_path,
        ]);

        let output = cmd.output().await?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to create GIF: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Clean up palette file
        let _ = tokio::fs::remove_file("palette.png").await;

        info!("GIF created: {}", output_path);
        Ok(())
    }

    pub async fn list_recordings(&self) -> Result<Vec<RecordingSession>> {
        Ok(self.active_recordings.values().cloned().collect())
    }

    async fn start_mp4_recording(
        &self,
        recording_id: &str,
        output_path: &str,
        display: &str,
    ) -> Result<()> {
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args([
            "-f",
            "x11grab",
            "-s",
            "1920x1080",
            "-r",
            "30",
            "-i",
            display,
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "23",
            "-y",
            output_path,
        ]);

        cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;

        info!("MP4 recording started: {}", recording_id);
        Ok(())
    }

    async fn start_webm_recording(
        &self,
        recording_id: &str,
        output_path: &str,
        display: &str,
    ) -> Result<()> {
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args([
            "-f",
            "x11grab",
            "-s",
            "1920x1080",
            "-r",
            "30",
            "-i",
            display,
            "-c:v",
            "libvpx-vp9",
            "-b:v",
            "1M",
            "-y",
            output_path,
        ]);

        cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;

        info!("WebM recording started: {}", recording_id);
        Ok(())
    }

    async fn start_gif_recording(
        &self,
        recording_id: &str,
        _output_path: &str,
        display: &str,
    ) -> Result<()> {
        // For GIF, we first record as MP4 then convert
        let temp_path = format!("/tmp/kvirtualstage-{}.mp4", recording_id);

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args([
            "-f",
            "x11grab",
            "-s",
            "1920x1080",
            "-r",
            "10", // Lower framerate for GIF
            "-i",
            display,
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-y",
            &temp_path,
        ]);

        cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;

        info!("GIF recording started (temp file): {}", recording_id);
        Ok(())
    }

    async fn find_ffmpeg() -> Result<PathBuf> {
        // Try common locations for ffmpeg
        let locations = vec![
            "/usr/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/opt/homebrew/bin/ffmpeg",
            "ffmpeg", // In PATH
        ];

        for location in locations {
            if let Ok(output) = Command::new(location).arg("-version").output().await {
                if output.status.success() {
                    info!("Found ffmpeg at: {}", location);
                    return Ok(PathBuf::from(location));
                }
            }
        }

        Err(anyhow!(
            "ffmpeg not found. Please install ffmpeg to enable recording functionality"
        ))
    }
}
