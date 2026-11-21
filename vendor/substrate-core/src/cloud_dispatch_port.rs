//! CloudDispatchPort — submit remote cloud-agent tasks and harvest PR results.
//!
//! Core defines the port contract and value types; `cloud-*` adapter crates
//! implement it against Cursor Cloud Agents, Kilo gateway-backed dispatch, etc.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Opaque handle returned by [`CloudDispatchPort::submit_task`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudTaskHandle {
    /// Adapter-local task identifier (stable for poll/harvest).
    pub id: String,
}

/// Observed lifecycle state of a cloud-dispatched task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudTaskStatus {
    /// Task accepted but not yet executing.
    Queued,
    /// Remote agent is actively working.
    Running,
    /// Task completed successfully; [`CloudDispatchPort::harvest`] may be called.
    Succeeded,
    /// Task failed; optional adapter message for diagnostics.
    Failed {
        /// Human-readable failure reason when available.
        message: Option<String>,
    },
}

/// Normalized harvest payload after a successful cloud run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudResult {
    /// Pull-request URL when the adapter opened one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    /// Branch containing the agent's work.
    pub branch: String,
    /// Short summary of changes (commit message, agent reply, or diff stat).
    pub diff_summary: String,
}

/// Outbound port for harness-agnostic cloud agent dispatch.
///
/// Lifecycle: [`submit_task`](CloudDispatchPort::submit_task) enqueues work and
/// returns a handle; [`poll_status`](CloudDispatchPort::poll_status) observes
/// progress; [`harvest`](CloudDispatchPort::harvest) collects the PR outcome
/// once status is [`CloudTaskStatus::Succeeded`].
#[async_trait]
pub trait CloudDispatchPort: Send + Sync {
    /// Submit a remote task against `repo` on base ref `branch` with `prompt`.
    async fn submit_task(&self, repo: &str, branch: &str, prompt: &str) -> Result<CloudTaskHandle>;

    /// Poll the current status for `handle`.
    async fn poll_status(&self, handle: &CloudTaskHandle) -> Result<CloudTaskStatus>;

    /// Harvest the normalized result for a succeeded task.
    async fn harvest(&self, handle: &CloudTaskHandle) -> Result<CloudResult>;
}
