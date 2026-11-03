// SPDX-License-Identifier: Apache-2.0 OR MIT
//! SQLite persistence layer for teamcomm daemon.
//!
//! Provides a `Store` struct that reads/writes sessions, reservations,
//! inbox messages, and live state to a SQLite database. Designed for
//! production deployments where in-memory state doesn't survive restarts.
//!
//! Thread-safe via `rusqlite::Connection` behind a `Mutex`.
//! Migration-aware via `PRAGMA user_version`.

use rusqlite::{params, Connection, Result as SqlResult};
use std::sync::Mutex;

use teamcomm_protocol::inbox::{InboxMessage, Priority};
use teamcomm_protocol::reservation::{Reservation, ReservationMode};
use teamcomm_protocol::session::{AgentType, Session};
use teamcomm_protocol::state::{AgentStatus, LiveState};

/// A SQLite-backed store for all teamcomm daemon state.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (or create) a database at `path`, applying migrations.
    pub fn open(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let store = Store {
            conn: Mutex::new(conn),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    /// Open an in-memory database (testing / temp-only).
    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Store {
            conn: Mutex::new(conn),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    // ─── Migrations ───────────────────────────────────────────

    fn apply_migrations(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                agent_type TEXT NOT NULL,
                pid INTEGER NOT NULL,
                working_dir TEXT NOT NULL DEFAULT '',
                capabilities TEXT NOT NULL DEFAULT '[]',
                started_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_heartbeat TEXT NOT NULL DEFAULT (datetime('now')),
                status TEXT NOT NULL DEFAULT 'idle',
                focus_file TEXT,
                focus_branch TEXT,
                worktree TEXT,
                current_task TEXT NOT NULL DEFAULT '',
                progress_pct INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS reservations (
                reservation_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                path TEXT NOT NULL,
                mode TEXT NOT NULL DEFAULT 'exclusive',
                acquired_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id)
            );

            CREATE TABLE IF NOT EXISTS inbox (
                message_id TEXT PRIMARY KEY,
                from_session TEXT NOT NULL,
                to_session TEXT NOT NULL,
                subject TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '',
                priority TEXT NOT NULL DEFAULT 'normal',
                ts TEXT NOT NULL DEFAULT (datetime('now')),
                is_read INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (from_session) REFERENCES sessions(session_id),
                FOREIGN KEY (to_session) REFERENCES sessions(session_id)
            );

            CREATE INDEX IF NOT EXISTS idx_inbox_to ON inbox(to_session);
            CREATE INDEX IF NOT EXISTS idx_inbox_from ON inbox(from_session);
            CREATE INDEX IF NOT EXISTS idx_reservations_sid ON reservations(session_id);
            ",
        )?;

        Ok(())
    }

    // ─── Sessions ─────────────────────────────────────────────

    /// Upsert a session record. `agent_type` and `capabilities` are JSON-serialized.
    pub fn upsert_session(
        &self,
        session: &Session,
        status: &str,
        focus_file: Option<&str>,
        focus_branch: Option<&str>,
        worktree: Option<&str>,
        current_task: &str,
        progress_pct: u32,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let agent_type_json = serde_json::to_string(&session.agent_type).unwrap_or_default();
        let capabilities_json =
            serde_json::to_string(&session.capabilities).unwrap_or_else(|_| "[]".into());
        let wd_str = session.working_dir.to_string_lossy().to_string();

        conn.execute(
            "INSERT INTO sessions (session_id, agent_type, pid, working_dir, capabilities, started_at, last_heartbeat, status, focus_file, focus_branch, worktree, current_task, progress_pct)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(session_id) DO UPDATE SET
                agent_type = excluded.agent_type,
                pid = excluded.pid,
                working_dir = excluded.working_dir,
                capabilities = excluded.capabilities,
                last_heartbeat = excluded.last_heartbeat,
                status = excluded.status,
                focus_file = excluded.focus_file,
                focus_branch = excluded.focus_branch,
                worktree = excluded.worktree,
                current_task = excluded.current_task,
                progress_pct = excluded.progress_pct",
            params![
                session.session_id,
                agent_type_json,
                session.pid,
                wd_str,
                capabilities_json,
                session.started_at.to_rfc3339(),
                session.last_heartbeat.to_rfc3339(),
                status,
                focus_file,
                focus_branch,
                worktree,
                current_task,
                progress_pct,
            ],
        )?;
        Ok(())
    }

    /// Simple upsert with just status fields (no per-field focus).
    pub fn upsert_session_status(
        &self,
        session_id: &str,
        status: &str,
        current_task: &str,
        progress_pct: u32,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET status = ?1, current_task = ?2, progress_pct = ?3, last_heartbeat = ?4 WHERE session_id = ?5",
            params![status, current_task, progress_pct, chrono::Utc::now().to_rfc3339(), session_id],
        )?;
        Ok(())
    }

    /// Update focus fields for a session (from state.set).
    pub fn upsert_session_focus(
        &self,
        session_id: &str,
        focus_file: Option<&str>,
        focus_branch: Option<&str>,
        worktree: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET focus_file = ?1, focus_branch = ?2, worktree = ?3, last_heartbeat = ?4 WHERE session_id = ?5",
            params![focus_file, focus_branch, worktree, chrono::Utc::now().to_rfc3339(), session_id],
        )?;
        Ok(())
    }

    /// Delete a session by ID (cascades to inbox + reservations).
    pub fn delete_session(&self, session_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM inbox WHERE from_session = ?1 OR to_session = ?1", params![session_id])?;
        conn.execute("DELETE FROM reservations WHERE session_id = ?1", params![session_id])?;
        conn.execute("DELETE FROM sessions WHERE session_id = ?1", params![session_id])?;
        Ok(())
    }

    /// Fetch a session by ID, reconstructing the Session struct.
    pub fn get_session(&self, session_id: &str) -> SqlResult<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, agent_type, pid, working_dir, capabilities, started_at, last_heartbeat FROM sessions WHERE session_id = ?1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        match rows.next()? {
            Some(row) => {
                let session_id: String = row.get(0)?;
                let agent_type_str: String = row.get(1)?;
                let pid: i64 = row.get(2)?;
                let wd_str: String = row.get(3)?;
                let capabilities_json: String = row.get(4)?;
                let started_at_str: String = row.get(5)?;
                let last_hb_str: String = row.get(6)?;

                let agent_type: AgentType =
                    serde_json::from_str(&agent_type_str).unwrap_or(AgentType::Custom(agent_type_str));
                let capabilities: Vec<String> =
                    serde_json::from_str(&capabilities_json).unwrap_or_default();

                let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let last_heartbeat = chrono::DateTime::parse_from_rfc3339(&last_hb_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(Some(Session {
                    session_id,
                    agent_type,
                    pid: pid as u32,
                    started_at,
                    working_dir: std::path::PathBuf::from(wd_str),
                    capabilities,
                    last_heartbeat,
                }))
            }
            None => Ok(None),
        }
    }

    /// Return all session IDs + agent_type JSON.
    pub fn list_sessions(&self) -> SqlResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT session_id, agent_type FROM sessions")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return status fields for a session (for discover.agents).
    pub fn get_session_status(&self, session_id: &str) -> SqlResult<Option<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT status, focus_file, current_task FROM sessions WHERE session_id = ?1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        match rows.next()? {
            Some(row) => {
                let status: String = row.get(0)?;
                let focus_file: Option<String> = row.get(1)?;
                let current_task: String = row.get(2)?;
                Ok(Some((status, focus_file.unwrap_or_default(), current_task)))
            }
            None => Ok(None),
        }
    }

    /// Prune sessions that haven't heartbeated since `cutoff` (ISO 8601).
    pub fn prune_sessions(&self, cutoff: &str) -> SqlResult<u64> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM sessions WHERE last_heartbeat < ?1",
            params![cutoff],
        )?;
        Ok(count as u64)
    }

    // ─── Reservations ─────────────────────────────────────────

    /// Upsert a reservation. `mode` is JSON-serialized from ReservationMode enum.
    pub fn upsert_reservation(&self, res: &Reservation) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let mode_str = serde_json::to_string(&res.mode).unwrap_or_else(|_| "\"exclusive\"".into());

        conn.execute(
            "INSERT INTO reservations (reservation_id, session_id, path, mode, acquired_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(reservation_id) DO UPDATE SET
                session_id = excluded.session_id,
                path = excluded.path,
                mode = excluded.mode,
                expires_at = excluded.expires_at",
            params![
                res.reservation_id,
                res.session_id,
                res.path.to_string_lossy().to_string(),
                mode_str,
                res.acquired_at.to_rfc3339(),
                res.expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_reservation(&self, reservation_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM reservations WHERE reservation_id = ?1",
            params![reservation_id],
        )?;
        Ok(())
    }

    /// List all reservation IDs for a session.
    pub fn list_reservations_for_session(&self, session_id: &str) -> SqlResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT reservation_id FROM reservations WHERE session_id = ?1")?;
        let rows = stmt.query_map(params![session_id], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Prune expired reservations (expires_at < cutoff).
    pub fn prune_reservations(&self, cutoff: &str) -> SqlResult<u64> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM reservations WHERE expires_at < ?1",
            params![cutoff],
        )?;
        Ok(count as u64)
    }

    // ─── Inbox ────────────────────────────────────────────────

    pub fn upsert_message(&self, msg: &InboxMessage) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let priority_str = serde_json::to_string(&msg.priority).unwrap_or_else(|_| "\"normal\"".into());

        conn.execute(
            "INSERT INTO inbox (message_id, from_session, to_session, subject, body, priority, ts, is_read)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(message_id) DO NOTHING",
            params![
                msg.message_id,
                msg.from_session,
                msg.to_session,
                msg.subject,
                msg.body,
                priority_str,
                msg.ts.to_rfc3339(),
                msg.read as i32,
            ],
        )?;
        Ok(())
    }

    pub fn mark_message_read(&self, message_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE inbox SET is_read = 1 WHERE message_id = ?1",
            params![message_id],
        )?;
        Ok(())
    }

    pub fn list_messages_for_session(&self, session_id: &str) -> SqlResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT message_id FROM inbox WHERE to_session = ?1 ORDER BY ts DESC")?;
        let rows = stmt.query_map(params![session_id], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Delete all messages for a session (used on deregister).
    pub fn delete_messages_for_session(&self, session_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM inbox WHERE from_session = ?1 OR to_session = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    // ─── Health ────────────────────────────────────────────────

    pub fn is_healthy(&self) -> bool {
        self.conn.lock().is_ok()
    }

    pub fn row_counts(&self) -> SqlResult<(u64, u64, u64)> {
        let conn = self.conn.lock().unwrap();
        let sessions: u64 =
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        let reservations: u64 =
            conn.query_row("SELECT COUNT(*) FROM reservations", [], |row| row.get(0))?;
        let inbox: u64 =
            conn.query_row("SELECT COUNT(*) FROM inbox", [], |row| row.get(0))?;
        Ok((sessions, reservations, inbox))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_ts() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 6, 12, 10, 0, 0).unwrap()
    }

    #[test]
    fn test_in_memory_create() {
        let store = Store::in_memory().expect("in-memory store");
        assert!(store.is_healthy());
    }

    #[test]
    fn test_session_crud() {
        let store = Store::in_memory().expect("in-memory store");
        let session = Session {
            session_id: "sess_test123".into(),
            agent_type: AgentType::Codex,
            pid: 42000,
            started_at: sample_ts(),
            working_dir: "/tmp/proj".into(),
            capabilities: vec!["rust".into(), "git:write".into()],
            last_heartbeat: sample_ts(),
        };

        store
            .upsert_session(&session, "idle", None, None, None, "", 0)
            .expect("upsert");
        let got = store.get_session("sess_test123").expect("get");
        assert!(got.is_some());
        assert_eq!(got.as_ref().unwrap().agent_type, AgentType::Codex);
        assert_eq!(got.as_ref().unwrap().capabilities.len(), 2);

        store.delete_session("sess_test123").expect("delete");
        assert!(store.get_session("sess_test123").unwrap().is_none());
    }

    #[test]
    fn test_inbox_message_crud() {
        let store = Store::in_memory().expect("in-memory store");
        let session = Session {
            session_id: "sess_a".into(),
            agent_type: AgentType::Codex,
            pid: 1,
            started_at: sample_ts(),
            working_dir: "/tmp".into(),
            capabilities: vec![],
            last_heartbeat: sample_ts(),
        };
        store
            .upsert_session(&session, "idle", None, None, None, "", 0)
            .expect("session");

        let msg = InboxMessage {
            message_id: "msg_1".into(),
            from_session: "sess_a".into(),
            to_session: "sess_a".into(),
            subject: "heads up".into(),
            body: "test".into(),
            priority: Priority::High,
            ts: sample_ts(),
            read: false,
        };
        store.upsert_message(&msg).expect("upsert msg");

        let msgs = store.list_messages_for_session("sess_a").expect("list");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], "msg_1");

        store.mark_message_read("msg_1").expect("mark read");
        store.delete_messages_for_session("sess_a").expect("delete");
        let msgs = store.list_messages_for_session("sess_a").expect("list");
        assert_eq!(msgs.len(), 0);
    }

    #[test]
    fn test_reservation_crud() {
        let store = Store::in_memory().expect("in-memory store");
        let session = Session {
            session_id: "sess_r".into(),
            agent_type: AgentType::Forge,
            pid: 2,
            started_at: sample_ts(),
            working_dir: "/tmp".into(),
            capabilities: vec![],
            last_heartbeat: sample_ts(),
        };
        store
            .upsert_session(&session, "idle", None, None, None, "", 0)
            .expect("session");

        let res = Reservation {
            reservation_id: "res_1".into(),
            session_id: "sess_r".into(),
            path: "/tmp/proj/src/lib.rs".into(),
            mode: ReservationMode::Write,
            acquired_at: sample_ts(),
            expires_at: sample_ts(),
        };
        store.upsert_reservation(&res).expect("upsert res");

        let reservations = store
            .list_reservations_for_session("sess_r")
            .expect("list");
        assert_eq!(reservations.len(), 1);
        assert_eq!(reservations[0], "res_1");

        store.delete_reservation("res_1").expect("delete");
        let reservations = store
            .list_reservations_for_session("sess_r")
            .expect("list");
        assert_eq!(reservations.len(), 0);
    }

    #[test]
    fn test_session_status_and_focus() {
        let store = Store::in_memory().expect("in-memory store");
        let session = Session {
            session_id: "sess_f".into(),
            agent_type: AgentType::Codex,
            pid: 3,
            started_at: sample_ts(),
            working_dir: "/tmp".into(),
            capabilities: vec![],
            last_heartbeat: sample_ts(),
        };
        store
            .upsert_session(&session, "working", Some("src/lib.rs"), None, None, "refactor", 50)
            .expect("upsert with focus");

        let (status, focus, task) = store
            .get_session_status("sess_f")
            .expect("get status")
            .expect("some");
        assert_eq!(status, "working");
        assert_eq!(focus, "src/lib.rs");
        assert_eq!(task, "refactor");
    }

    #[test]
    fn test_row_counts() {
        let store = Store::in_memory().expect("in-memory store");
        let session = Session {
            session_id: "sess_c".into(),
            agent_type: AgentType::Codex,
            pid: 4,
            started_at: sample_ts(),
            working_dir: "/tmp".into(),
            capabilities: vec![],
            last_heartbeat: sample_ts(),
        };
        store
            .upsert_session(&session, "idle", None, None, None, "", 0)
            .expect("session");
        let (s, r, i) = store.row_counts().expect("counts");
        assert_eq!(s, 1);
        assert_eq!(r, 0);
        assert_eq!(i, 0);
    }
}
