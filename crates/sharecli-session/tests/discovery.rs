// SPDX-License-Identifier: Apache-2.0 OR MIT
// FR:003 — Agent session discovery test suite
use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use sharecli_session::{
    scan_and_record, AgentSession, ObservationKind, ProcessEvidence, SessionObservation,
    SessionStateProvider, SessionStore, SurfaceAdapter, SurfaceCapabilities, SurfaceRecord,
};

#[derive(Clone)]
struct FakeAdapter {
    surfaces: Vec<SurfaceRecord>,
    capability_error_ids: Vec<String>,
}

impl SurfaceAdapter for FakeAdapter {
    fn capabilities(&self, surface: &SurfaceRecord) -> Result<SurfaceCapabilities> {
        if self.capability_error_ids.iter().any(|id| id == &surface.id) {
            return Err(anyhow!("capabilities unavailable for {}", surface.id));
        }
        Ok(SurfaceCapabilities { read: true, write: true, resize: true, ..Default::default() })
    }

    fn discover(&self) -> Result<Vec<SurfaceRecord>> {
        Ok(self.surfaces.clone())
    }
}

#[derive(Default)]
struct FakeState {
    ids: HashMap<String, String>,
}

impl SessionStateProvider for FakeState {
    fn session_id(&self, surface: &SurfaceRecord, _harness: &str) -> Result<Option<String>> {
        Ok(self.ids.get(&surface.id).cloned())
    }
}

fn surface(id: &str, argv: &[&str]) -> SurfaceRecord {
    SurfaceRecord {
        id: id.to_string(),
        terminal: "ghostty".to_string(),
        title: Some(id.to_string()),
        cwd: PathBuf::from("/tmp/project"),
        process: Some(ProcessEvidence {
            pid: Some(42),
            tty: Some("ttys001".to_string()),
            cwd: PathBuf::from("/tmp/project"),
            argv: argv.iter().map(|value| (*value).to_string()).collect(),
            started_at: Some("2026-08-01T00:00:00Z".to_string()),
        }),
    }
}

#[test]
fn scan_resolves_codex_argv_and_materializes_session() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![surface("pane-1", &["codex", "resume", "codex-1"])],
        capability_error_ids: vec![],
    };

    let report =
        scan_and_record(&adapter, &FakeState::default(), &store, "2026-08-01T01:00:00Z").unwrap();

    assert_eq!(report.scanned, 1);
    assert_eq!(report.recorded, 1);
    assert!(report.failures.is_empty());
    assert_eq!(report.results[0].kind, ObservationKind::Discovered);
    assert_eq!(report.results[0].session_id.as_deref(), Some("codex-1"));
    assert_eq!(store.list().unwrap(), vec![AgentSession::codex("codex-1", "/tmp/project")]);
}

#[test]
fn state_id_is_used_and_corroborated_by_process_argv() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![surface("pane-2", &["forge", "--conversation-id", "forge-2"])],
        capability_error_ids: vec![],
    };
    let state = FakeState { ids: HashMap::from([("pane-2".to_string(), "forge-2".to_string())]) };

    let report = scan_and_record(&adapter, &state, &store, "2026-08-01T01:00:00Z").unwrap();

    assert_eq!(report.results[0].session_id.as_deref(), Some("forge-2"));
    assert_eq!(
        store.list().unwrap()[0].confidence,
        sharecli_session::ResolutionConfidence::Corroborated
    );
}

#[test]
fn unknown_process_is_recorded_without_unsafe_resume_recipe() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![surface("pane-3", &["zsh", "-i"])],
        capability_error_ids: vec![],
    };

    let report =
        scan_and_record(&adapter, &FakeState::default(), &store, "2026-08-01T01:00:00Z").unwrap();

    assert_eq!(report.recorded, 1);
    assert_eq!(report.results[0].session_id, None);
    assert!(store.list().unwrap().is_empty());
    let observations = store.observations(Some("pane-3")).unwrap();
    assert_eq!(observations.len(), 1);
    assert!(observations[0].session.is_none());
}

#[test]
fn repeated_scan_marks_surface_updated() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![surface("pane-4", &["kilo", "--session", "kilo-4"])],
        capability_error_ids: vec![],
    };
    let state = FakeState::default();

    let first = scan_and_record(&adapter, &state, &store, "2026-08-01T01:00:00Z").unwrap();
    let second = scan_and_record(&adapter, &state, &store, "2026-08-01T01:01:00Z").unwrap();

    assert_eq!(first.results[0].kind, ObservationKind::Discovered);
    assert_eq!(second.results[0].kind, ObservationKind::Updated);
    assert_eq!(store.observations(Some("pane-4")).unwrap().len(), 2);
}

#[test]
fn one_capability_failure_does_not_discard_other_surfaces() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![
            surface("pane-bad", &["codex", "resume", "bad"]),
            surface("pane-good", &["opencode", "--session", "good"]),
        ],
        capability_error_ids: vec!["pane-bad".to_string()],
    };

    let report =
        scan_and_record(&adapter, &FakeState::default(), &store, "2026-08-01T01:00:00Z").unwrap();

    assert_eq!(report.scanned, 2);
    assert_eq!(report.recorded, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].surface_id, "pane-bad");
    assert_eq!(store.list().unwrap()[0].session_id, "good");
}

#[test]
fn malformed_timestamp_is_rejected_before_discovery() {
    let store = SessionStore::open_memory().unwrap();
    let adapter = FakeAdapter {
        surfaces: vec![surface("pane-6", &["codex", "resume", "id"])],
        capability_error_ids: vec![],
    };

    let error = scan_and_record(&adapter, &FakeState::default(), &store, " ").unwrap_err();

    assert!(error.to_string().contains("observed_at"));
    assert!(store.observations(None).unwrap().is_empty());
}

#[test]
fn observation_type_remains_serializable() {
    let observation = SessionObservation::new(
        "2026-08-01T01:00:00Z",
        surface("pane-7", &["codex", "resume", "id"]),
        None,
        SurfaceCapabilities::default(),
        ObservationKind::Discovered,
    );
    let encoded = serde_json::to_string(&observation).unwrap();
    assert!(encoded.contains("pane-7"));
}
