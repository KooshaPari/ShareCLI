//! Durable observation records for terminal surfaces and agent sessions.

use serde::{Deserialize, Serialize};

use crate::{AgentSession, SurfaceRecord};

/// Capabilities advertised by a terminal surface adapter.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceCapabilities {
    /// Adapter can read the surface's current pane contents.
    pub read: bool,
    /// Adapter can inject input / paste.
    pub write: bool,
    /// Adapter can request a resize on the underlying pty.
    pub resize: bool,
    /// Adapter can snapshot / restore the pane topology.
    pub layout: bool,
    /// Adapter owns a durable PTY that survives the parent process.
    pub durable_pty: bool,
}

/// Why an observation was appended.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ObservationKind {
    /// A new surface was first observed.
    Discovered,
    /// An existing surface changed (focus, title, cwd, etc).
    Updated,
    /// The surface's process has exited.
    Exited,
    /// The session was rebuilt from a previous observation.
    Recovered,
}

/// Append-only state observed from a terminal surface.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionObservation {
    /// Stable observation sequence; assigned at write time.
    #[serde(default)]
    pub seq: i64,
    /// RFC 3339 timestamp the observation was captured at.
    pub observed_at: String,
    /// Surface this observation describes.
    pub surface: SurfaceRecord,
    /// Optional resolved agent session, when an adapter could identify one.
    pub session: Option<AgentSession>,
    /// Surface capabilities at observation time.
    pub capabilities: SurfaceCapabilities,
    /// Why this observation was recorded.
    pub kind: ObservationKind,
}

impl SessionObservation {
    /// Build a new observation with `seq = 0`; the store assigns the real seq on append.
    pub fn new(
        observed_at: impl Into<String>,
        surface: SurfaceRecord,
        session: Option<AgentSession>,
        capabilities: SurfaceCapabilities,
        kind: ObservationKind,
    ) -> Self {
        Self { seq: 0, observed_at: observed_at.into(), surface, session, capabilities, kind }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{AgentSession, ProcessEvidence, ResolutionConfidence, SessionState, SessionStore};

    fn surface(id: &str) -> SurfaceRecord {
        SurfaceRecord {
            id: id.to_string(),
            terminal: "ghostty".to_string(),
            title: Some(id.to_string()),
            cwd: PathBuf::from("/tmp/project"),
            process: Some(ProcessEvidence {
                pid: Some(42),
                tty: Some("ttys001".to_string()),
                cwd: PathBuf::from("/tmp/project"),
                argv: vec!["codex".to_string()],
                started_at: Some("1".to_string()),
            }),
        }
    }

    #[test]
    fn observation_survives_store_reopen_and_materializes_session() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("sharecli-ledger-{suffix}.sqlite"));
        let session = AgentSession::codex("session-1", "/tmp/project");
        let observation = SessionObservation::new(
            "2026-07-31T00:00:00Z",
            surface("ghostty:1"),
            Some(session.clone()),
            SurfaceCapabilities { read: true, write: true, ..Default::default() },
            ObservationKind::Discovered,
        );
        let store = SessionStore::open(&path).unwrap();
        let seq = store.append_observation(&observation).unwrap();
        assert_eq!(seq, 1);
        drop(store);
        let reopened = SessionStore::open(&path).unwrap();
        assert_eq!(reopened.observations(None).unwrap().len(), 1);
        assert_eq!(reopened.get(&session.id).unwrap(), Some(session));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn heuristic_session_is_not_auto_resumable() {
        let mut session = AgentSession::codex("session-2", "/tmp/project");
        session.confidence = ResolutionConfidence::Heuristic;
        session.state = SessionState::Unknown;
        assert!(!session.auto_resumable());
    }

    #[test]
    fn compaction_keeps_latest_observation_per_surface() {
        let store = SessionStore::open_memory().unwrap();
        for (surface_id, timestamp) in [("a", "1"), ("a", "2"), ("b", "3")] {
            store
                .append_observation(&SessionObservation::new(
                    timestamp,
                    surface(surface_id),
                    None,
                    SurfaceCapabilities::default(),
                    ObservationKind::Updated,
                ))
                .unwrap();
        }
        assert_eq!(store.compact_observations().unwrap(), 1);
        let observations = store.observations(None).unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].surface.id, "a");
        assert_eq!(observations[0].observed_at, "2");
    }
}
