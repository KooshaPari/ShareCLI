// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR:011 / C10 - automatic recovery uses only fresh, latest surface evidence.

use std::path::PathBuf;

use chrono::{Duration, Utc};
use sharecli_session::{
    AgentSession, ObservationKind, SessionObservation, SessionService, SessionStore,
    SurfaceCapabilities, SurfaceRecord,
};

fn observation(
    surface_id: &str,
    observed_at: &str,
    session: Option<AgentSession>,
    kind: ObservationKind,
) -> SessionObservation {
    SessionObservation::new(
        observed_at,
        SurfaceRecord {
            id: surface_id.to_string(),
            terminal: "ghostty".to_string(),
            title: None,
            cwd: PathBuf::from("/tmp/project"),
            process: None,
        },
        session,
        SurfaceCapabilities::default(),
        kind,
    )
}

#[test]
fn recovery_plan_keeps_only_fresh_latest_surface_sessions() {
    let store = SessionStore::open_memory().unwrap();
    let now = Utc::now();
    let fresh = (now - Duration::minutes(5)).to_rfc3339();
    let old = (now - Duration::hours(5)).to_rfc3339();

    store
        .append_observation(&observation(
            "surface-replaced",
            &fresh,
            Some(AgentSession::codex("old-id", "/tmp/project")),
            ObservationKind::Discovered,
        ))
        .unwrap();
    store
        .append_observation(&observation(
            "surface-replaced",
            &fresh,
            None,
            ObservationKind::Updated,
        ))
        .unwrap();
    store
        .append_observation(&observation(
            "surface-stale",
            &old,
            Some(AgentSession::forge("stale-id", "/tmp/project")),
            ObservationKind::Discovered,
        ))
        .unwrap();
    store
        .append_observation(&observation(
            "surface-live",
            &fresh,
            Some(AgentSession::opencode("live-id", "/tmp/project")),
            ObservationKind::Discovered,
        ))
        .unwrap();
    store.upsert(&AgentSession::codex("legacy-only", "/tmp/project")).unwrap();

    let plan = SessionService::new(store).recovery_plan(Duration::hours(1)).unwrap();

    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].session_id, "live-id");
}

#[test]
fn recovery_plan_rejects_malformed_observation_time() {
    let store = SessionStore::open_memory().unwrap();
    store
        .append_observation(&observation(
            "surface-invalid-time",
            "not-a-timestamp",
            Some(AgentSession::codex("invalid-time", "/tmp/project")),
            ObservationKind::Discovered,
        ))
        .unwrap();

    assert!(SessionService::new(store).recovery_plan(Duration::hours(1)).unwrap().is_empty());
}

#[test]
fn future_observation_cannot_replace_current_surface_evidence() {
    let store = SessionStore::open_memory().unwrap();
    let now = Utc::now();
    store
        .append_observation(&observation(
            "surface-clock-skew",
            &(now - Duration::minutes(5)).to_rfc3339(),
            Some(AgentSession::codex("current-id", "/tmp/project")),
            ObservationKind::Discovered,
        ))
        .unwrap();
    store
        .append_observation(&observation(
            "surface-clock-skew",
            &(now + Duration::hours(1)).to_rfc3339(),
            Some(AgentSession::codex("future-id", "/tmp/project")),
            ObservationKind::Updated,
        ))
        .unwrap();

    let plan = SessionService::new(store).recovery_plan(Duration::hours(1)).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].session_id, "current-id");
}

#[test]
fn delayed_older_observation_cannot_replace_newer_surface_evidence() {
    let store = SessionStore::open_memory().unwrap();
    let now = Utc::now();
    store
        .append_observation(&observation(
            "surface-delayed",
            &(now - Duration::minutes(2)).to_rfc3339(),
            Some(AgentSession::codex("newer-id", "/tmp/project")),
            ObservationKind::Updated,
        ))
        .unwrap();
    store
        .append_observation(&observation(
            "surface-delayed",
            &(now - Duration::minutes(10)).to_rfc3339(),
            Some(AgentSession::codex("older-id", "/tmp/project")),
            ObservationKind::Updated,
        ))
        .unwrap();

    let plan = SessionService::new(store).recovery_plan(Duration::hours(1)).unwrap();
    assert_eq!(plan.iter().map(|s| s.session_id.as_str()).collect::<Vec<_>>(), ["newer-id"]);
}
