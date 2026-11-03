// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli mesh …` — Maildir task-queue operator surface (FR-010).
//!
//! Exposes [`MaildirQueue::status`] and [`MaildirQueue::reclaim_owner`] so
//! operators can inspect and recover stranded in-flight work without
//! reaching into `new/` / `cur/` by hand.

use std::path::Path;

use anyhow::{Context, Result};
use sharecli_mesh::MaildirQueue;

/// Print Maildir depth (`ready` / `in_flight` / `pending`).
///
/// When `json` is true, emit a single JSON object matching [`sharecli_mesh::MaildirStatus`].
pub fn status(queue: &Path, json: bool) -> Result<()> {
    let q = MaildirQueue::open(queue)
        .with_context(|| format!("open mesh queue {}", queue.display()))?;
    let st = q.status().context("mesh queue status")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&st)?);
        return Ok(());
    }
    println!("mesh queue: {}", st.path.display());
    println!("  ready:     {}", st.ready);
    println!("  in_flight: {}", st.in_flight);
    println!("  pending:   {}", st.pending);
    Ok(())
}

/// Reclaim `cur/` tasks owned by `owner` back to `new/`.
pub fn reclaim(queue: &Path, owner: &str) -> Result<()> {
    if owner.trim().is_empty() {
        anyhow::bail!("mesh reclaim requires a non-empty --owner");
    }
    let q = MaildirQueue::open(queue)
        .with_context(|| format!("open mesh queue {}", queue.display()))?;
    let n = q.reclaim_owner(owner).with_context(|| format!("reclaim owner '{owner}'"))?;
    println!("reclaimed {n} task(s) for owner '{owner}'");
    Ok(())
}
