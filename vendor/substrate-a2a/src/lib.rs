//! # a2a
//!
//! A2A (agent-to-agent) schema types for the substrate mailbox and tasklist.
//!
//! This crate is schema-only: no engine adapters, no transport, no IO.
//! Types map to the A2A protocol shape but are transport-agnostic.
//!
//! # Task lifecycle
//!
//! `Submitted -> Working -> InputRequired -> Working -> Completed` with
//! `Failed` and `Cancelled` reachable from any non-terminal state.
//! Use [`TaskState::can_transition`] to validate moves.
//!
//! # Message lifecycle
//!
//! `Unread -> Delivered -> Consumed`. Atomic claim semantics are enforced
//! by the store layer (e.g. `store-sqlite` via `UPDATE WHERE state='unread'`).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
/// Message, Part, MsgState, MessageKind, and Artifact types.
pub mod message;
/// Task and TaskState types.
pub mod task;

pub use error::{A2aError, A2aResult};
pub use message::{Artifact, Message, MessageKind, MsgState, Part};
pub use task::{Task, TaskState};
