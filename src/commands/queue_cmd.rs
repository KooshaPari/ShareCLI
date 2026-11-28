// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli queue …` — first-class Maildir task-queue verbs (FR-010).
//!
//! Read/write surface over [`sharecli_mesh::MaildirQueue`], sharing the queue
//! root with `sharecli mesh` via
//! [`sharecli_mesh::resolve_mesh_queue_path`] / `SHARECLI_MESH_QUEUE`.
//!
//! Deliberately **no executor**: `queue drain --apply` claims ready tasks and
//! stops there. Nothing is spawned or run from this module.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use sharecli_mesh::{resolve_mesh_queue_path, MaildirQueue, TaskEnvelope};

use crate::error::SharecliError;

/// Payload preview width (`queue list` column / `payload_preview` JSON key).
const PREVIEW_LEN: usize = 60;

/// Resolve the queue root shared with `sharecli mesh`: an explicit `--queue`
/// wins, otherwise `SHARECLI_MESH_QUEUE` or the per-user state-dir default.
pub fn resolve_root(explicit: Option<&Path>) -> PathBuf {
    explicit.map_or_else(resolve_mesh_queue_path, Path::to_path_buf)
}

fn open(root: &Path) -> Result<MaildirQueue> {
    MaildirQueue::open(root).with_context(|| format!("open mesh queue {}", root.display()))
}

fn usage(message: impl Into<String>) -> anyhow::Error {
    SharecliError::user_input(message).into()
}

/// One-line preview of a payload: whitespace collapsed, first [`PREVIEW_LEN`] chars.
fn payload_preview(payload: &Value) -> String {
    let raw = serde_json::to_string(payload).unwrap_or_default();
    let one_line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(PREVIEW_LEN).collect()
}

fn created_label(created_at: f64) -> String {
    let secs = created_at as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| secs.to_string())
}

/// Stable per-task JSON object (`location` = `new` or `cur`).
fn task_json(envelope: &TaskEnvelope, location: &str) -> Value {
    json!({
        "id": envelope.id,
        "priority": envelope.priority,
        "created_at": envelope.created_at,
        "created": created_label(envelope.created_at),
        "attempts": envelope.attempts,
        "owner": envelope.owner,
        "payload_preview": payload_preview(&envelope.payload),
        "payload": envelope.payload,
        "location": location,
    })
}

/// `queue stats` — same data as `sharecli mesh status`.
pub fn stats(queue: Option<&Path>, json_out: bool) -> Result<()> {
    let root = resolve_root(queue);
    let status = open(&root)?.status().context("queue status")?;
    if json_out {
        println!("{}", serde_json::to_string_pretty(&json!({"ok": true, "queue": status}))?);
        return Ok(());
    }
    // Same renderer `mesh status` uses: one implementation, two entry points.
    crate::commands::mesh::print_status(&status, false)
}

/// `queue list` — every envelope in `new/` then `cur/`.
pub fn list(queue: Option<&Path>, json_out: bool) -> Result<()> {
    let root = resolve_root(queue);
    let queue = open(&root)?;
    let ready = queue.list_ready()?;
    let in_flight = queue.list_in_flight()?;

    let rows: Vec<(&TaskEnvelope, &str)> =
        ready.iter().map(|e| (e, "new")).chain(in_flight.iter().map(|e| (e, "cur"))).collect();

    if json_out {
        let tasks: Vec<Value> = rows.iter().map(|(e, loc)| task_json(e, loc)).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "queue": root,
                "count": tasks.len(),
                "ready": ready.len(),
                "in_flight": in_flight.len(),
                "tasks": tasks,
            }))?
        );
        return Ok(());
    }

    println!("queue: {}", root.display());
    println!(
        "{:<36}  {:<3}  {:<21}  {:<8}  {:<12}  {:<6}  {}",
        "ID", "PRI", "CREATED", "ATTEMPTS", "OWNER", "WHERE", "PAYLOAD"
    );
    for (envelope, location) in &rows {
        println!(
            "{:<36}  {:<3}  {:<21}  {:<8}  {:<12}  {:<6}  {}",
            envelope.id,
            envelope.priority,
            created_label(envelope.created_at),
            envelope.attempts,
            envelope.owner.as_deref().unwrap_or("-"),
            location,
            payload_preview(&envelope.payload),
        );
    }
    println!("count: {} (ready {}, in-flight {})", rows.len(), ready.len(), in_flight.len());
    Ok(())
}

/// `queue enqueue` — write a payload at `priority` (0–9, lower first).
///
/// `--json` here carries the payload object; machine-readable *output* is
/// `--emit-json` so the two meanings never collide.
pub fn enqueue(
    queue: Option<&Path>,
    payload_json: Option<&str>,
    payload_file: Option<&Path>,
    priority: u8,
    emit_json: bool,
) -> Result<()> {
    let payload = match (payload_json, payload_file) {
        (Some(_), Some(_)) => {
            return Err(usage(
                "queue enqueue accepts either --json '<object>' or --payload-file <path>, not both",
            ))
        }
        (None, None) => {
            return Err(usage("queue enqueue requires --json '<object>' or --payload-file <path>"))
        }
        (Some(raw), None) => serde_json::from_str(raw)
            .map_err(|e| usage(format!("--json payload is not valid JSON: {e}")))?,
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read payload file {}", path.display()))?;
            serde_json::from_str(&text).map_err(|e| {
                usage(format!("payload file {} is not valid JSON: {e}", path.display()))
            })?
        }
    };

    let root = resolve_root(queue);
    let id = open(&root)?.enqueue(payload, priority).context("queue enqueue")?;

    if emit_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "id": id,
                "priority": priority,
                "queue": root,
            }))?
        );
    } else {
        println!("{id}");
    }
    Ok(())
}

/// `queue claim` — claim the highest-priority ready task (rc 0, even when empty).
pub fn claim(queue: Option<&Path>, owner: Option<&str>, json_out: bool) -> Result<()> {
    let root = resolve_root(queue);
    let owner = owner.map(str::trim).filter(|o| !o.is_empty());
    let claimed = open(&root)?.claim(owner).context("queue claim")?;

    if json_out {
        let task = claimed.as_ref().map(|e| task_json(e, "cur"));
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "queue": root,
                "claimed": task.is_some(),
                "task": task,
            }))?
        );
        return Ok(());
    }

    match claimed {
        Some(envelope) => {
            println!("claimed task {}", envelope.id);
            println!("  priority: {}", envelope.priority);
            println!("  created:  {}", created_label(envelope.created_at));
            println!("  attempts: {}", envelope.attempts);
            println!("  owner:    {}", envelope.owner.as_deref().unwrap_or("-"));
            println!("  payload:  {}", serde_json::to_string(&envelope.payload)?);
        }
        None => println!("no ready task"),
    }
    Ok(())
}

/// `queue drain` — report-only by default; `--apply` performs the claims.
pub fn drain(queue: Option<&Path>, apply: bool, json_out: bool) -> Result<()> {
    let root = resolve_root(queue);
    let queue = open(&root)?;
    let before = queue.status().context("queue status before drain")?;
    let ready = queue.list_ready()?;

    let mut claimed: Vec<TaskEnvelope> = Vec::new();
    if apply {
        while let Some(envelope) = queue.claim(None).context("queue drain claim")? {
            claimed.push(envelope);
        }
    }
    let after = queue.status().context("queue status after drain")?;

    let tasks: Vec<Value> = if apply {
        claimed.iter().map(|e| task_json(e, "cur")).collect()
    } else {
        ready.iter().map(|e| task_json(e, "new")).collect()
    };

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "queue": root,
                "applied": apply,
                "count": tasks.len(),
                "counts": {"before": before, "after": after},
                "tasks": tasks,
            }))?
        );
        return Ok(());
    }

    if apply {
        println!("drained {} task(s)", tasks.len());
    } else {
        println!(
            "drain dry-run: {} task(s) would be claimed in this order (pass --apply to claim)",
            tasks.len()
        );
    }
    let listed: &[TaskEnvelope] = if apply { &claimed } else { &ready };
    for (index, envelope) in listed.iter().enumerate() {
        println!(
            "  {}. {} priority={} attempts={} {}",
            index + 1,
            envelope.id,
            envelope.priority,
            envelope.attempts,
            payload_preview(&envelope.payload)
        );
    }
    println!(
        "before: ready {} in-flight {} | after: ready {} in-flight {}",
        before.ready, before.in_flight, after.ready, after.in_flight
    );
    Ok(())
}
