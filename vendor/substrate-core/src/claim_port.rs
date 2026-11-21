//! ClaimPort — atomic work-queue with fuzzy near-duplicate detection.
//!
//! Core defines the contract; `store-sqlite` implements BEGIN IMMEDIATE CAS
//! claiming and strsim-backed deduplication.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lifecycle state of a queued work item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkItemState {
    /// Waiting to be claimed.
    Pending,
    /// Claimed by a worker.
    Claimed,
    /// Finished.
    Completed,
}

/// A unit of claimable work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    /// Stable identity.
    pub id: Uuid,
    /// Named queue (tenant/lane).
    pub queue: String,
    /// Payload text used for fuzzy dedup.
    pub body: String,
    /// Current state.
    pub state: WorkItemState,
    /// Worker that holds the claim, if any.
    pub claimed_by: Option<String>,
}

/// Atomic work-queue port.
///
/// Implementations MUST guarantee that at most one worker wins a concurrent
/// claim race for a given item (`BEGIN IMMEDIATE` + conditional `UPDATE`).
pub trait ClaimPort: Send + Sync {
    /// Error type returned by store operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Enqueue a new pending item; returns its id.
    ///
    /// Returns an error if `body` is a near-duplicate of an existing pending
    /// or claimed item in the same queue.
    fn enqueue(&self, queue: &str, body: &str) -> Result<Uuid, Self::Error>;

    /// Atomically claim the oldest pending item in `queue` for `worker_id`.
    ///
    /// Returns `None` when the queue is empty.
    fn claim_next(&self, queue: &str, worker_id: &str) -> Result<Option<WorkItem>, Self::Error>;

    /// Returns `true` if `body` is a near-duplicate of an existing item.
    fn is_near_duplicate(&self, queue: &str, body: &str) -> Result<bool, Self::Error>;
}
