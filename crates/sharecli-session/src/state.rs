//! Explicit launch-time state evidence for harness session recovery.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{SessionStateProvider, SurfaceRecord};

/// One deliberate surface-to-harness mapping written by a launcher or wrapper.
///
/// The surface id is mandatory. A PID, when present, prevents a stale mapping
/// from being applied after a terminal surface has been recycled.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SidecarRecord {
    /// Surface id the record maps.
    pub surface_id: String,
    /// Harness binary name (e.g. `codex`, `forge`).
    pub harness: String,
    /// Stable harness-side session identifier.
    pub session_id: String,
    /// Optional launcher pid; used to reject stale records.
    #[serde(default)]
    pub pid: Option<u32>,
}

/// Append one exact launch-time mapping to a sidecar, creating it owner-only.
pub fn append_record(path: &Path, record: &SidecarRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create sidecar directory {}", parent.display()))?;
    }
    let mut line = serde_json::to_vec(record).context("serialize sidecar record")?;
    line.push(b'\n');
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file =
        options.open(path).with_context(|| format!("open sidecar {}", path.display()))?;
    #[cfg(unix)]
    {
        let mut permissions = file
            .metadata()
            .with_context(|| format!("stat sidecar {}", path.display()))?
            .permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o600);
        file.set_permissions(permissions)
            .with_context(|| format!("protect sidecar {}", path.display()))?;
    }
    file.write_all(&line).with_context(|| format!("append sidecar {}", path.display()))?;
    file.sync_data().with_context(|| format!("sync sidecar {}", path.display()))?;
    Ok(())
}

/// Read-only JSONL provider for exact launch-time session mappings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidecarStateProvider {
    path: PathBuf,
}

impl SidecarStateProvider {
    /// Construct a provider. The file is read on each lookup so a long-running
    /// watcher observes wrapper registrations without a restart.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path the provider reads on every lookup.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn records(&self) -> Result<Vec<SidecarRecord>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read sidecar {}", self.path.display()))
            }
        };
        text.lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(line_number, line)| {
                serde_json::from_str(line).with_context(|| {
                    format!("parse sidecar {} line {}", self.path.display(), line_number + 1)
                })
            })
            .collect()
    }
}

impl SessionStateProvider for SidecarStateProvider {
    fn session_id(&self, surface: &SurfaceRecord, harness: &str) -> Result<Option<String>> {
        let pid = surface.process.as_ref().and_then(|process| process.pid);
        let mut match_record = None;
        for record in self.records()? {
            if record.surface_id != surface.id || record.harness != harness {
                continue;
            }
            // JSONL is append-only: the last record for a surface/harness is
            // authoritative. A newer PID mismatch must not fall back to an
            // older mapping from a recycled process.
            match_record = Some(record);
        }
        Ok(match_record.and_then(|record| {
            if record.pid.is_some() && record.pid != pid {
                return None;
            }
            (!record.session_id.trim().is_empty()).then_some(record.session_id)
        }))
    }
}
