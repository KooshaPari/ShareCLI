//! Errors that may cross an a2a boundary.

use thiserror::Error;

/// Errors produced by wire-schema validation.
#[derive(Debug, Error)]
pub enum A2aError {
    /// A lifecycle transition was rejected by the FSM.
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        /// State we attempted to move away from.
        from: crate::task::TaskState,
        /// State we attempted to move to.
        to: crate::task::TaskState,
    },

    /// A message-state transition was rejected.
    #[error("invalid message-state transition: {from:?} -> {to:?}")]
    InvalidMessageTransition {
        /// State we attempted to move away from.
        from: crate::message::MsgState,
        /// State we attempted to move to.
        to: crate::message::MsgState,
    },

    /// A message could not be claimed (already held).
    #[error("claim conflict: {0}")]
    ClaimConflict(String),

    /// Serialization / deserialization failed.
    #[error("serde error: {0}")]
    Serde(String),
}

impl From<serde_json::Error> for A2aError {
    fn from(e: serde_json::Error) -> Self {
        A2aError::Serde(e.to_string())
    }
}

/// Convenience result alias.
pub type A2aResult<T> = std::result::Result<T, A2aError>;
