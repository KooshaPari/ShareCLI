//! The single error type crossing every port boundary.

use thiserror::Error;

/// Errors that may cross a substrate port boundary.
///
/// Adapters map their concrete failure modes (IO, process spawn, parse,
/// network) onto these variants so that the core/application layers never
/// depend on adapter-specific error types.
#[derive(Debug, Error)]
pub enum SubstrateError {
    /// A requested entity (task, conversation, session) was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A lifecycle transition was rejected by the FSM.
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        /// State we attempted to move away from.
        from: crate::domain::TaskState,
        /// State we attempted to move to.
        to: crate::domain::TaskState,
    },

    /// An atomic claim/lease could not be acquired (already held).
    #[error("claim conflict: {0}")]
    ClaimConflict(String),

    /// A backing engine (CLI/process) failed.
    #[error("engine error: {0}")]
    Engine(String),

    /// A transport (mailbox/bus) operation failed.
    #[error("transport error: {0}")]
    Transport(String),

    /// A store (persistence) operation failed.
    #[error("store error: {0}")]
    Store(String),

    /// Routing could not select a target.
    #[error("routing error: {0}")]
    Routing(String),

    /// Serialization / deserialization failed.
    #[error("serde error: {0}")]
    Serde(String),

    /// An IO error (file, pipe, process).
    #[error("io error: {0}")]
    Io(String),

    /// A schedule expression or trigger is invalid.
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),

    /// A workflow graph contains a cycle.
    #[error("workflow cycle: {0}")]
    CycleDetected(String),

    /// Enqueue rejected because the body is a near-duplicate.
    #[error("duplicate work: {0}")]
    DuplicateWork(String),

    /// Input failed JSON schema validation for a skill/tool.
    #[error("schema validation: {0}")]
    SchemaValidation(String),

    /// A memory port operation failed.
    #[error("memory error: {0}")]
    Memory(String),

    /// A managed subprocess operation failed.
    #[error("process error: {0}")]
    Process(String),

    /// A filesystem watcher operation failed.
    #[error("watcher error: {0}")]
    Watcher(String),

    /// A cloud-dispatch adapter operation failed.
    #[error("cloud dispatch error: {0}")]
    CloudDispatch(String),

    /// A catch-all for adapter-specific failures that do not fit above.
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for SubstrateError {
    fn from(e: serde_json::Error) -> Self {
        SubstrateError::Serde(e.to_string())
    }
}

/// Convenience result alias used across ports.
pub type Result<T> = std::result::Result<T, SubstrateError>;
