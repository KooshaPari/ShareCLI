//! MailboxStore port — durable mailbox and tasklist backed by a concrete store.
//!
//! This port is implemented by `store-sqlite`; the core crate only defines the
//! contract. No IO, no adapter dependencies here.
//!
//! The port uses its own mirror types so that `substrate-core` stays free of
//! any dependency on `a2a` or adapter crates. `store-sqlite` maps between the
//! `a2a` types and these port-level types internally.

use uuid::Uuid;

/// Opaque task state at the port boundary.
///
/// Adapters convert to/from `a2a::TaskState` internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxTaskState {
    /// Submitted, not yet started.
    Submitted,
    /// Actively being worked on.
    Working,
    /// Waiting for caller input.
    InputRequired,
    /// Finished successfully.
    Completed,
    /// Ended in error.
    Failed,
    /// Explicitly cancelled.
    Cancelled,
}

/// The MailboxStore port: durable mailbox + task list.
///
/// Implementations MUST guarantee atomic claim semantics: at most one caller
/// wins the race to claim a given message (i.e. `Unread → Delivered` is
/// exclusive).
pub trait MailboxStore: Send + Sync {
    /// The message type stored by this implementation.
    type Msg;
    /// The task type stored by this implementation.
    type Task;
    /// The error type returned by store operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Post a message into the mailbox.
    fn post(&self, msg: &Self::Msg) -> Result<(), Self::Error>;

    /// Return all unread messages addressed to `to` in `team_id`.
    fn inbox(&self, team_id: &str, to: &str) -> Result<Vec<Self::Msg>, Self::Error>;

    /// Atomic claim: transition message state from `Unread` to `Delivered`.
    ///
    /// Returns `true` iff this caller won the race (SQLite rowcount == 1).
    fn claim(&self, message_id: Uuid) -> Result<bool, Self::Error>;

    /// Mark a message as `Consumed`.
    fn consume(&self, message_id: Uuid) -> Result<(), Self::Error>;

    /// Roll back a claim: transition message state from `Delivered` to `Unread`.
    fn unclaim(&self, message_id: Uuid) -> Result<(), Self::Error>;

    /// Insert a new task.
    fn task_create(&self, task: &Self::Task) -> Result<(), Self::Error>;

    /// Advance a task's state, optionally recording a note.
    fn task_update(
        &self,
        id: Uuid,
        state: MailboxTaskState,
        note: Option<&str>,
    ) -> Result<(), Self::Error>;

    /// Return all tasks for a team.
    fn task_list(&self, team_id: &str) -> Result<Vec<Self::Task>, Self::Error>;
}
