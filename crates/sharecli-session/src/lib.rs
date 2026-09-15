#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_lists_sessions() {
        let store = SessionStore::open_memory().unwrap();
        let session = AgentSession::new("codex", "abc", "/tmp/project");
        store.upsert(&session).unwrap();
        let rows = store.list().unwrap();
        assert_eq!(rows, vec![session]);
    }

    #[test]
    fn rpc_inspect_and_recovery_plan_are_typed() {
        let store = SessionStore::open_memory().unwrap();
        let session = AgentSession::new("forge", "def", "/tmp/work");
        store.upsert(&session).unwrap();
        let service = SessionService::new(store);
        let inspect = service.inspect(&session.id).unwrap().unwrap();
        assert_eq!(inspect.resume.harness, "forge");
        assert!(service.recovery_plan(chrono::Duration::hours(1)).unwrap().is_empty());
    }

    #[test]
    fn harness_recipes_and_confidence_are_explicit() {
        assert_eq!(
            AgentSession::forge("id", "/tmp").resume.argv,
            vec!["forge", "--conversation-id", "id"]
        );
        assert_eq!(AgentSession::codex("id", "/tmp").resume.argv, vec!["codex", "resume", "id"]);
        assert_eq!(
            AgentSession::opencode("id", "/tmp").resume.argv,
            vec!["opencode", "--session", "id"]
        );
        assert_eq!(AgentSession::kilo("id", "/tmp").resume.argv, vec!["kilo", "--session", "id"]);
        assert_eq!(
            AgentSession::cursor("id", "/tmp").resume.argv,
            vec!["cursor-agent", "--resume", "id"]
        );
        let session = AgentSession::codex("id", "/tmp");
        assert_eq!(session.confidence, ResolutionConfidence::Exact);
        assert_eq!(session.state, SessionState::Active);
    }

    // --- Additional coverage tests ---

    #[test]
    fn auto_resumable_exact_with_nonempty_session_id_and_argv() {
        let session = AgentSession::codex("id", "/tmp");
        assert!(session.auto_resumable());
    }

    #[test]
    fn auto_resumable_corroborated() {
        let mut session = AgentSession::codex("id", "/tmp");
        session.confidence = ResolutionConfidence::Corroborated;
        assert!(session.auto_resumable());
    }

    #[test]
    fn auto_resumable_heuristic_is_not() {
        let mut session = AgentSession::codex("id", "/tmp");
        session.confidence = ResolutionConfidence::Heuristic;
        assert!(!session.auto_resumable());
    }

    #[test]
    fn auto_resumable_unavailable_is_not() {
        let mut session = AgentSession::codex("id", "/tmp");
        session.confidence = ResolutionConfidence::Unavailable;
        assert!(!session.auto_resumable());
    }

    #[test]
    fn auto_resumable_empty_session_id_is_not() {
        let mut session = AgentSession::codex("id", "/tmp");
        session.resume.session_id = String::new();
        assert!(!session.auto_resumable());
    }

    #[test]
    fn auto_resumable_empty_argv_is_not() {
        let mut session = AgentSession::codex("id", "/tmp");
        session.resume.argv.clear();
        assert!(!session.auto_resumable());
    }

    #[test]
    fn agent_session_new_format() {
        let session = AgentSession::new("claude", "abc", "/tmp/project");
        assert_eq!(session.id, "claude:abc");
        assert_eq!(session.harness, "claude");
        assert_eq!(session.session_id, "abc");
        assert_eq!(session.cwd, std::path::PathBuf::from("/tmp/project"));
        assert_eq!(session.resume.harness, "claude");
        assert_eq!(session.resume.session_id, "abc");
        assert_eq!(session.resume.cwd, std::path::PathBuf::from("/tmp/project"));
        assert_eq!(session.resume.argv, vec!["claude", "resume", "abc"]);
    }

    #[test]
    fn session_state_eq() {
        assert_eq!(SessionState::Pending, SessionState::Pending);
        assert_ne!(SessionState::Pending, SessionState::Active);
        assert_ne!(SessionState::Active, SessionState::Exited);
        assert_ne!(SessionState::Exited, SessionState::Unknown);
    }

    #[test]
    fn resolution_confidence_eq() {
        assert_eq!(ResolutionConfidence::Exact, ResolutionConfidence::Exact);
        assert_ne!(ResolutionConfidence::Exact, ResolutionConfidence::Corroborated);
        assert_ne!(ResolutionConfidence::Corroborated, ResolutionConfidence::Heuristic);
        assert_ne!(ResolutionConfidence::Heuristic, ResolutionConfidence::Unavailable);
    }

    #[test]
    fn surface_record_clone_eq() {
        let record = SurfaceRecord {
            id: "s1".into(),
            terminal: "ghostty".into(),
            title: Some("main".into()),
            cwd: std::path::PathBuf::from("/tmp"),
            process: None,
        };
        let cloned = record.clone();
        assert_eq!(record, cloned);
    }

    #[test]
    fn process_evidence_clone_eq() {
        let ev = ProcessEvidence {
            pid: Some(1234),
            tty: Some("ttys001".into()),
            cwd: std::path::PathBuf::from("/tmp"),
            argv: vec!["bash".into()],
            started_at: Some("2026-01-01".into()),
        };
        let cloned = ev.clone();
        assert_eq!(ev, cloned);
    }
}

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};

pub mod adapter;
pub mod discovery;
pub mod events;
pub mod layout;
pub mod ledger;
pub mod recovery;
pub mod resolver;
pub mod rpc;
pub mod state;

pub use adapter::{GhosttySurfaceAdapter, SurfaceAdapter, SurfaceIo, ZmxSurfaceAdapter};
pub use discovery::{
    scan_and_record, DiscoveryFailure, DiscoveryReport, DiscoveryResult, MapStateProvider,
    NoStateProvider, SessionStateProvider, SurfaceObservationScanner,
};
pub use events::{
    SurfaceEventError, SurfaceEventHub, SurfaceEventKind, SurfaceEventNotification,
    SurfaceEventParams, SurfaceSubscribeAck, SurfaceSubscribeRequest,
    SurfaceSubscriptionCapabilities, MAX_EVENT_CHUNK_BYTES, MAX_EVENT_QUEUE_CAPACITY,
};
pub use layout::{LayoutAxis, LayoutNode, LayoutRestoreItem, LayoutRestoreReport, LayoutSnapshot};
pub use ledger::{ObservationKind, SessionObservation, SurfaceCapabilities};
pub use recovery::{validate_recipe, RecoveryExecutor, RecoveryOutcome, RecoveryResult};
pub use resolver::{resolve as resolve_session, EvidenceSource, Resolution};
pub use state::{append_record, SidecarRecord, SidecarStateProvider};

/// Default freshness window for automatic recovery plans.
pub const DEFAULT_RECOVERY_MAX_AGE_SECONDS: u64 = 4 * 60 * 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResumeRecipe {
    pub harness: String,
    pub session_id: String,
    pub cwd: PathBuf,
    pub argv: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResolutionConfidence {
    Exact,
    Corroborated,
    Heuristic,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SessionState {
    Pending,
    Active,
    Exited,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessEvidence {
    pub pid: Option<u32>,
    pub tty: Option<String>,
    pub cwd: PathBuf,
    pub argv: Vec<String>,
    pub started_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceRecord {
    pub id: String,
    pub terminal: String,
    pub title: Option<String>,
    pub cwd: PathBuf,
    pub process: Option<ProcessEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSession {
    pub id: String,
    pub harness: String,
    pub session_id: String,
    pub cwd: PathBuf,
    pub resume: ResumeRecipe,
    pub confidence: ResolutionConfidence,
    pub state: SessionState,
}

impl AgentSession {
    /// Whether this record has enough evidence for unattended recovery.
    pub fn auto_resumable(&self) -> bool {
        matches!(self.confidence, ResolutionConfidence::Exact | ResolutionConfidence::Corroborated)
            && !self.resume.session_id.is_empty()
            && !self.resume.argv.is_empty()
    }
}

impl AgentSession {
    pub fn new(
        harness: impl Into<String>,
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        let harness = harness.into();
        let session_id = session_id.into();
        let cwd = cwd.into();
        let id = format!("{harness}:{session_id}");
        let argv = vec![harness.clone(), "resume".to_string(), session_id.clone()];
        Self {
            id,
            harness: harness.clone(),
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            resume: ResumeRecipe { harness, session_id, cwd, argv },
            confidence: ResolutionConfidence::Exact,
            state: SessionState::Active,
        }
    }

    pub fn forge(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self::with_recipe("forge", session_id, cwd, |id| {
            vec!["forge".into(), "--conversation-id".into(), id]
        })
    }
    pub fn codex(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self::with_recipe("codex", session_id, cwd, |id| vec!["codex".into(), "resume".into(), id])
    }
    pub fn opencode(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self::with_recipe("opencode", session_id, cwd, |id| {
            vec!["opencode".into(), "--session".into(), id]
        })
    }
    pub fn kilo(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self::with_recipe("kilo", session_id, cwd, |id| vec!["kilo".into(), "--session".into(), id])
    }
    pub fn cursor(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self::with_recipe("cursor-agent", session_id, cwd, |id| {
            vec!["cursor-agent".into(), "--resume".into(), id]
        })
    }
    fn with_recipe<F>(
        harness: &str,
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        build: F,
    ) -> Self
    where
        F: FnOnce(String) -> Vec<String>,
    {
        let session_id = session_id.into();
        let argv = build(session_id.clone());
        let cwd = cwd.into();
        Self {
            id: format!("{harness}:{session_id}"),
            harness: harness.to_string(),
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            resume: ResumeRecipe { harness: harness.to_string(), session_id, cwd, argv },
            confidence: ResolutionConfidence::Exact,
            state: SessionState::Active,
        }
    }
}

pub struct SessionStore {
    conn: Mutex<Connection>,
}

impl SessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("create session database directory {}", parent.display())
            })?;
        }
        let conn = Connection::open(path).context("open session database")?;
        Self::init(conn)
    }
    pub fn open_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }
    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, harness TEXT NOT NULL, session_id TEXT NOT NULL, cwd TEXT NOT NULL, resume_json TEXT NOT NULL, confidence TEXT NOT NULL, state TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS session_observations (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                observed_at TEXT NOT NULL,
                surface_id TEXT NOT NULL,
                surface_json TEXT NOT NULL,
                session_json TEXT,
                capabilities_json TEXT NOT NULL,
                kind TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS session_observations_surface_seq
                ON session_observations(surface_id, seq);
             CREATE INDEX IF NOT EXISTS session_observations_time
                ON session_observations(observed_at);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }
    pub fn upsert(&self, session: &AgentSession) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        Self::upsert_locked(&conn, session)
    }
    pub fn list(&self) -> Result<Vec<AgentSession>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let mut stmt =
            conn.prepare("SELECT id,harness,session_id,cwd,resume_json,confidence,state FROM sessions ORDER BY id")?;
        let rows = stmt.query_map([], Self::row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Build a recovery plan from the newest fresh observation per surface.
    pub fn recovery_plan(&self, max_age: Duration) -> Result<Vec<AgentSession>> {
        if max_age <= Duration::zero() {
            tracing::error!(?max_age, "recovery plan requested with non-positive max age");
            anyhow::bail!("recovery max age must be positive");
        }
        let now = Utc::now();
        let cutoff = now - max_age;
        let mut latest: BTreeMap<String, (DateTime<Utc>, i64, SessionObservation)> =
            BTreeMap::new();
        for observation in self.observations(None)? {
            let observed_at = match DateTime::parse_from_rfc3339(&observation.observed_at) {
                Ok(value) => value.with_timezone(&Utc),
                Err(error) => {
                    tracing::warn!(
                        surface_id = %observation.surface.id,
                        observed_at = %observation.observed_at,
                        error = %error,
                        "ignoring observation with malformed timestamp"
                    );
                    continue;
                }
            };
            if observed_at > now {
                tracing::warn!(
                    surface_id = %observation.surface.id,
                    observed_at = %observed_at,
                    "ignoring future-dated observation"
                );
                continue;
            }
            if observed_at < cutoff {
                tracing::debug!(
                    surface_id = %observation.surface.id,
                    observed_at = %observed_at,
                    "ignoring stale observation"
                );
                continue;
            }
            let surface_id = observation.surface.id.clone();
            let candidate = (observed_at, observation.seq, observation);
            let replace = latest
                .get(&surface_id)
                .is_none_or(|current| (candidate.0, candidate.1) > (current.0, current.1));
            if replace {
                latest.insert(surface_id, candidate);
            }
        }
        let mut sessions = BTreeMap::new();
        for (_, _, observation) in latest.into_values() {
            if observation.kind == ObservationKind::Exited {
                continue;
            }
            let Some(session) = observation.session else { continue };
            if !session.auto_resumable() {
                continue;
            }
            sessions.insert(session.id.clone(), session);
        }
        Ok(sessions.into_values().collect())
    }
    pub fn get(&self, id: &str) -> Result<Option<AgentSession>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let mut stmt =
            conn.prepare("SELECT id,harness,session_id,cwd,resume_json,confidence,state FROM sessions WHERE id=?1")?;
        let mut rows = stmt.query([id])?;
        rows.next()?.map(Self::row).transpose().map_err(Into::into)
    }

    /// Append an observation and atomically refresh the materialized session row.
    pub fn append_observation(&self, observation: &SessionObservation) -> Result<i64> {
        let mut conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO session_observations
             (observed_at, surface_id, surface_json, session_json, capabilities_json, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                observation.observed_at,
                observation.surface.id,
                serde_json::to_string(&observation.surface)?,
                observation.session.as_ref().map(serde_json::to_string).transpose()?,
                serde_json::to_string(&observation.capabilities)?,
                serde_json::to_string(&observation.kind)?,
            ],
        )?;
        let seq = tx.last_insert_rowid();
        if let Some(session) = &observation.session {
            Self::upsert_locked(&tx, session)?;
        }
        tx.commit()?;
        Ok(seq)
    }

    /// Read observations in append order, optionally limited to one surface.
    pub fn observations(&self, surface_id: Option<&str>) -> Result<Vec<SessionObservation>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let mut stmt = if surface_id.is_some() {
            conn.prepare("SELECT seq, observed_at, surface_json, session_json, capabilities_json, kind FROM session_observations WHERE surface_id=?1 ORDER BY seq")?
        } else {
            conn.prepare("SELECT seq, observed_at, surface_json, session_json, capabilities_json, kind FROM session_observations ORDER BY seq")?
        };
        let rows = if let Some(surface_id) = surface_id {
            stmt.query_map([surface_id], Self::observation_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            stmt.query_map([], Self::observation_row)?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }

    /// Compact history while retaining the latest observation for every surface.
    pub fn compact_observations(&self) -> Result<usize> {
        let mut conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = tx.execute(
            "DELETE FROM session_observations
             WHERE seq NOT IN (SELECT MAX(seq) FROM session_observations GROUP BY surface_id)",
            [],
        )?;
        tx.commit()?;
        Ok(removed)
    }
    fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSession> {
        let cwd: String = row.get(3)?;
        let resume: String = row.get(4)?;
        let confidence: String = row.get(5)?;
        let state: String = row.get(6)?;
        Ok(AgentSession {
            id: row.get(0)?,
            harness: row.get(1)?,
            session_id: row.get(2)?,
            cwd: PathBuf::from(cwd),
            resume: serde_json::from_str(&resume).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            confidence: serde_json::from_str(&confidence).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            state: serde_json::from_str(&state).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        })
    }

    fn upsert_locked(conn: &rusqlite::Connection, session: &AgentSession) -> Result<()> {
        conn.execute(
            "INSERT INTO sessions (id,harness,session_id,cwd,resume_json,confidence,state)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET harness=excluded.harness,
               session_id=excluded.session_id, cwd=excluded.cwd,
               resume_json=excluded.resume_json, confidence=excluded.confidence,
               state=excluded.state",
            params![
                session.id,
                session.harness,
                session.session_id,
                session.cwd.to_string_lossy(),
                serde_json::to_string(&session.resume)?,
                serde_json::to_string(&session.confidence)?,
                serde_json::to_string(&session.state)?,
            ],
        )?;
        Ok(())
    }

    fn observation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionObservation> {
        let session_json: Option<String> = row.get(3)?;
        Ok(SessionObservation {
            seq: row.get(0)?,
            observed_at: row.get(1)?,
            surface: serde_json::from_str(&row.get::<_, String>(2)?).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            session: session_json.map(|value| serde_json::from_str(&value)).transpose().map_err(
                |e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                },
            )?,
            capabilities: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            kind: serde_json::from_str(&row.get::<_, String>(5)?).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        })
    }
}

pub struct SessionService {
    store: SessionStore,
}
impl SessionService {
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
    pub fn list(&self) -> Result<Vec<AgentSession>> {
        self.store.list()
    }
    pub fn inspect(&self, id: &str) -> Result<Option<AgentSession>> {
        self.store.get(id)
    }
    pub fn recovery_plan(&self, max_age: Duration) -> Result<Vec<AgentSession>> {
        self.store.recovery_plan(max_age)
    }
    pub fn observations(&self, surface_id: Option<&str>) -> Result<Vec<SessionObservation>> {
        self.store.observations(surface_id)
    }
    pub fn compact_observations(&self) -> Result<usize> {
        self.store.compact_observations()
    }
}
