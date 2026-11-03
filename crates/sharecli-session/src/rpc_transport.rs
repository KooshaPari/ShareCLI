// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Plain request/response Unix transport for the surface control protocol.

use super::*;

#[cfg(unix)]
pub async fn serve_surface_unix_with_token(
    path: &std::path::Path,
    control: Arc<dyn SurfaceControl>,
    expected_token: Option<String>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let control = control.clone();
        let expected_token = expected_token.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut input = reader;
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 16 * 1024];
            while let Ok(count) = input.read(&mut chunk).await {
                if count == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..count]);
                if buffer.len() > MAX_SURFACE_LINE_BYTES {
                    break;
                }
                while let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
                    let Ok(line) = std::str::from_utf8(&buffer[..newline]) else {
                        return;
                    };
                    let should_reply = serde_json::from_str::<Value>(line)
                        .ok()
                        .and_then(|request| {
                            request.as_object().map(|object| object.contains_key("id"))
                        })
                        .unwrap_or(false);
                    let response = dispatch_surface_with_token(
                        control.clone(),
                        line,
                        expected_token.as_deref(),
                    )
                    .await;
                    buffer.drain(..=newline);
                    if !should_reply {
                        continue;
                    }
                    let Ok(mut payload) = serde_json::to_string(&response) else {
                        return;
                    };
                    payload.push('\n');
                    if writer.write_all(payload.as_bytes()).await.is_err() {
                        return;
                    }
                }
            }
        });
    }
}
