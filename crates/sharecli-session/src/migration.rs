// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Schema migration scaffold for `sharecli-session` (C00 L3 / FR-SESSION-002).
//!
//! The session store uses SQLite with WAL mode. Migrations are numbered and
//! applied incrementally on startup. Each migration is idempotent and
//! version-gated via a `_meta` table.
//!
//! # Adding a new migration
//!
//! 1. Increment `CURRENT_VERSION`.
//! 2. Add a new `fn migrate_N(conn)` in the `run_migrations` match.
//! 3. Use `IF NOT EXISTS` / `ALTER TABLE ... ADD COLUMN` patterns that are
//!    safe to re-run if partially applied.
//! 4. Add a test in the `#[cfg(test)]` module below.

use anyhow::{Context, Result};
use rusqlite::Connection;

/// The latest schema version this binary knows how to migrate to.
/// When you add a new migration, bump this number.
pub const CURRENT_VERSION: i64 = 1;

/// Ensure the `_meta` table exists and return the current schema version.
fn current_version(conn: &Connection) -> Result<i64> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .context("ensure _meta table")?;

    let version: i64 = conn
        .query_row("SELECT value FROM _meta WHERE key = 'schema_version'", [], |row| row.get(0))
        .unwrap_or(0);
    Ok(version)
}

/// Record the new schema version in `_meta`.
fn set_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO _meta (key, value) VALUES ('schema_version', ?1)",
        [version.to_string()],
    )
    .context("set schema_version in _meta")?;
    Ok(())
}

/// Apply all pending migrations from `current` up to `CURRENT_VERSION`.
///
/// Each migration function must be idempotent — safe to re-run if the database
/// was partially migrated (e.g., process killed mid-migration). Use
/// `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`
/// (SQLite 3.37+) or catch-and-ignore patterns for DDL.
pub fn run_migrations(conn: &Connection) -> Result<i64> {
    let mut version = current_version(conn)?;

    if version >= CURRENT_VERSION {
        tracing::debug!(schema_version = version, "session schema up to date");
        return Ok(version);
    }

    tracing::info!(from = version, to = CURRENT_VERSION, "running session schema migrations");

    while version < CURRENT_VERSION {
        version = match version {
            0 => migrate_0_to_1(conn).context("migration 0 → 1")?,
            other => anyhow::bail!("unknown migration source version: {other}"),
        };
        set_version(conn, version)?;
    }

    tracing::info!(schema_version = version, "session schema migration complete");
    Ok(version)
}

// ---------------------------------------------------------------------------
// Migration 0 → 1: baseline schema
// ---------------------------------------------------------------------------

/// Baseline schema: `sessions` + `session_observations` tables.
///
/// This migration captures the schema that previously lived in `SessionStore::init`.
/// On a fresh database all tables are created; on an existing database that was
/// never versioned (version = 0) the `IF NOT EXISTS` clauses make this safe.
fn migrate_0_to_1(conn: &Connection) -> Result<i64> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id          TEXT PRIMARY KEY,
            harness     TEXT NOT NULL,
            session_id  TEXT NOT NULL,
            cwd         TEXT NOT NULL,
            resume_json TEXT NOT NULL,
            confidence  TEXT NOT NULL,
            state       TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS session_observations (
            seq              INTEGER PRIMARY KEY AUTOINCREMENT,
            observed_at      TEXT NOT NULL,
            surface_id       TEXT NOT NULL,
            surface_json     TEXT NOT NULL,
            session_json     TEXT,
            capabilities_json TEXT NOT NULL,
            kind             TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS session_observations_surface_seq
            ON session_observations(surface_id, seq);
        CREATE INDEX IF NOT EXISTS session_observations_time
            ON session_observations(observed_at);",
    )
    .context("create baseline session tables")?;
    Ok(1)
}

// ---------------------------------------------------------------------------
// Future migration template
// ---------------------------------------------------------------------------

// fn migrate_1_to_2(conn: &Connection) -> Result<i64> {
//     conn.execute_batch(
//         "ALTER TABLE sessions ADD COLUMN last_heartbeat TEXT;
//          -- future: add new indexes, views, etc."
//     )
//     .context("migration 1 → 2")?;
//     Ok(2)
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_store() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn fresh_database_applies_migrations() {
        let conn = in_memory_store();
        let version: i64 = conn
            .query_row("SELECT value FROM _meta WHERE key = 'schema_version'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[test]
    fn idempotent_on_existing_database() {
        let conn = Connection::open_in_memory().unwrap();
        let v1 = run_migrations(&conn).unwrap();
        let v2 = run_migrations(&conn).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v2, CURRENT_VERSION);
    }

    #[test]
    fn sessions_table_has_expected_columns() {
        let conn = in_memory_store();
        // Verify the table exists and has the right shape by inserting a row.
        conn.execute_batch(
            "INSERT INTO sessions (id, harness, session_id, cwd, resume_json, confidence, state)
             VALUES ('test:1', 'forge', 's1', '/tmp', '{}', 'Exact', 'Active')",
        )
        .unwrap();
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn observations_table_has_expected_columns() {
        let conn = in_memory_store();
        conn.execute_batch(
            "INSERT INTO session_observations
             (observed_at, surface_id, surface_json, capabilities_json, kind)
             VALUES ('2026-01-01T00:00:00Z', 's1', '{}', '{}', 'Active')",
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM session_observations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
