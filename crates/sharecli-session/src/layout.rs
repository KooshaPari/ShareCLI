//! Durable terminal layout snapshots and restore reports.

use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::SessionStore;

/// Direction in which a terminal surface is split.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LayoutAxis {
    /// Side-by-side panes (left/right).
    Horizontal,
    /// Stacked panes (top/bottom).
    Vertical,
}

/// Recursive terminal layout tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LayoutNode {
    /// A leaf terminal pane identified by its stable surface ID.
    Pane { surface_id: String },
    /// A binary split. `ratio_millis` is the first child's share in thousandths.
    Split { axis: LayoutAxis, ratio_millis: u16, children: Vec<LayoutNode> },
}

/// Durable snapshot of a terminal application's pane topology.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayoutSnapshot {
    /// Stable identifier (typically `<terminal>:<id>`).
    pub id: String,
    /// Terminal application name.
    pub terminal: String,
    /// RFC 3339 timestamp at which the snapshot was captured.
    pub captured_at: String,
    /// Root of the recursive pane tree.
    pub root: LayoutNode,
}

/// Per-surface result produced by a layout restore adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayoutRestoreItem {
    /// Surface id this item refers to.
    pub surface_id: String,
    /// Whether the adapter successfully restored the surface.
    pub restored: bool,
    /// Optional human-readable explanation (especially when `restored == false`).
    pub detail: Option<String>,
}

/// Typed report for a completed or partially completed layout restore.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayoutRestoreReport {
    /// Identifier of the layout snapshot this report restores.
    pub layout_id: String,
    /// One entry per surface the adapter attempted to restore.
    pub items: Vec<LayoutRestoreItem>,
}

impl LayoutSnapshot {
    /// Validate topology before persistence or adapter execution.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("layout id must not be empty");
        }
        if self.terminal.trim().is_empty() {
            anyhow::bail!("layout terminal must not be empty");
        }
        let mut surfaces = HashSet::new();
        validate_node(&self.root, &mut surfaces)
    }
}

fn validate_node(node: &LayoutNode, surfaces: &mut HashSet<String>) -> Result<()> {
    match node {
        LayoutNode::Pane { surface_id } => {
            if surface_id.trim().is_empty() {
                anyhow::bail!("layout surface id must not be empty");
            }
            if !surfaces.insert(surface_id.clone()) {
                anyhow::bail!("layout surface id appears more than once: {surface_id}");
            }
        }
        LayoutNode::Split { ratio_millis, children, .. } => {
            if !(1..=999).contains(ratio_millis) {
                anyhow::bail!("layout split ratio must be between 1 and 999");
            }
            if children.len() != 2 {
                anyhow::bail!("layout split must contain exactly two children");
            }
            for child in children {
                validate_node(child, surfaces)?;
            }
        }
    }
    Ok(())
}

impl SessionStore {
    /// Insert or replace one validated terminal layout snapshot.
    pub fn save_layout(&self, snapshot: &LayoutSnapshot) -> Result<()> {
        snapshot.validate()?;
        let mut conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_schema(&tx)?;
        tx.execute(
            "INSERT INTO layouts (id, terminal, captured_at, snapshot_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET terminal=excluded.terminal,
               captured_at=excluded.captured_at, snapshot_json=excluded.snapshot_json",
            params![
                snapshot.id,
                snapshot.terminal,
                snapshot.captured_at,
                serde_json::to_string(snapshot)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Load a layout snapshot by its durable ID.
    pub fn get_layout(&self, id: &str) -> Result<Option<LayoutSnapshot>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        ensure_schema(&conn)?;
        let json: Option<String> = conn
            .query_row("SELECT snapshot_json FROM layouts WHERE id=?1", [id], |row| row.get(0))
            .optional()?;
        json.map(|value| serde_json::from_str(&value).context("decode layout snapshot")).transpose()
    }

    /// List all stored layouts in stable ID order.
    pub fn list_layouts(&self) -> Result<Vec<LayoutSnapshot>> {
        let conn = self.conn.lock().map_err(|_| anyhow::anyhow!("session store poisoned"))?;
        ensure_schema(&conn)?;
        let mut stmt = conn.prepare("SELECT snapshot_json FROM layouts ORDER BY id")?;
        let layouts = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| {
                let json = row?;
                serde_json::from_str(&json).context("decode layout snapshot")
            })
            .collect();
        layouts
    }
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS layouts (
            id TEXT PRIMARY KEY,
            terminal TEXT NOT NULL,
            captured_at TEXT NOT NULL,
            snapshot_json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS layouts_terminal_captured
            ON layouts(terminal, captured_at);",
    )?;
    Ok(())
}
