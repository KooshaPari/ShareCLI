// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR:011 / C10 — session ledger durability: observations survive a store
//! reopen, and heuristic-confidence observations persist without being marked
//! auto-resumable.
use std::path::PathBuf;

use sharecli_session::{
    AgentSession, ObservationKind, ResolutionConfidence, SessionObservation, SessionStore,
    SurfaceCapabilities, SurfaceRecord,
};

fn observation(id: &str, session_id: &str, confidence: ResolutionConfidence) -> SessionObservation {
    let surface = SurfaceRecord {
        id: id.to_string(),
        terminal: "ghostty".to_string(),
        title: None,
        cwd: PathBuf::from("/tmp"),
        process: None,
    };
    let mut session = AgentSession::new("codex", session_id, "/tmp");
    session.confidence = confidence;
    SessionObservation::new(
        "2026-08-08T00:00:00Z",
        surface,
        Some(session),
        SurfaceCapabilities::default(),
        ObservationKind::Updated,
    )
}

#[test]
fn observations_survive_store_reopen() {
    let path = std::env::temp_dir().join(format!(
        "sharecli-session-ledger-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));

    {
        let store = SessionStore::open(&path).unwrap();
        store
            .append_observation(&observation("obs-1", "codex:abc", ResolutionConfidence::Exact))
            .unwrap();
    }

    let reopened = SessionStore::open(&path).unwrap();
    let rows = reopened.observations(None).unwrap();
    assert_eq!(rows.len(), 1);
    let session = rows[0].session.as_ref().expect("observation carries session");
    assert!(session.auto_resumable(), "Exact-confidence session must be auto-resumable");
    assert_eq!(session.confidence, ResolutionConfidence::Exact);

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn heuristic_observations_are_persisted_but_not_resumable() {
    let store = SessionStore::open_memory().unwrap();
    store
        .append_observation(&observation(
            "obs-heuristic",
            "codex:ambiguous",
            ResolutionConfidence::Heuristic,
        ))
        .unwrap();

    let rows = store.observations(None).unwrap();
    assert_eq!(rows.len(), 1);
    let session = rows[0].session.as_ref().expect("observation carries session");
    assert!(!session.auto_resumable(), "Heuristic-confidence session must not be auto-resumable");
    assert_eq!(session.confidence, ResolutionConfidence::Heuristic);
}
