// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `sharecli-mesh` — execution-substrate mesh primitives for sharecli.
//!
//! # Boundary
//!
//! Per `thegent` / sharecli boundary audit (2026-06-21), the Maildir task queue
//! is sharecli-owned execution substrate. Control-plane orchestration stays in
//! `thegent`; this crate ports `thegent.mesh.task_queue.MaildirQueue`,
//! `smart_merge.SmartMerger` (git fallback), and a lightweight `WorktreePool`.
//!
//! # Maildir lifecycle
//!
//! ```text
//! tmp/  — staging (write envelope)
//! new/  — ready to claim (atomic rename from tmp)
//! cur/  — claimed / in-flight (atomic rename from new)
//! ```
//!
//! Public API: [`MaildirQueue::enqueue`], [`MaildirQueue::claim`],
//! [`MaildirQueue::ack`], [`MaildirQueue::nack`], [`MaildirQueue::list_pending`],
//! [`MaildirQueue::status`], [`MaildirQueue::reclaim_owner`],
//! [`SmartMerger`], [`WorktreePool`].

mod operator_status;
mod smart_merge;
mod task_queue;
mod worktree_pool;

pub use operator_status::{capture_maildir_status, resolve_mesh_queue_path, MESH_QUEUE_ENV};
pub use smart_merge::{MergeResult, SmartMerger};
pub use task_queue::{MaildirQueue, MaildirStatus, TaskEnvelope};
pub use worktree_pool::{WorktreeLease, WorktreePool, WorktreePoolError};
