//! The five port traits — the only seams adapters may implement.
//!
//! Ports are expressed as `async_trait` object-safe traits returning
//! [`crate::error::Result`]. Core defines them; adapter crates implement them;
//! the application layer is generic over them. Core never implements a port.

use async_trait::async_trait;

use crate::{
    domain::{
        ConversationDump, EngineCapabilities, Mailbox, Message, RoutingDecision, StructuredResult,
        Task,
    },
    error::Result,
};

/// Drives a concrete agent engine (a CLI such as `forge`, or an SDK).
///
/// Lifecycle: [`start`](EnginePort::start) launches a run and yields a
/// [`Session`]. [`dump`](EnginePort::dump) exports the raw conversation, which
/// [`extract_result`](EnginePort::extract_result) normalizes into a
/// [`StructuredResult`].
#[async_trait]
pub trait EnginePort: Send + Sync {
    /// Start a new run for `task`, returning a live session handle.
    async fn start(&self, task: &Task) -> Result<crate::domain::Session>;

    /// Resume an existing conversation with a follow-up prompt.
    async fn resume(&self, conv_id: &str, prompt: &str) -> Result<crate::domain::Session>;

    /// Export the raw conversation for `conv_id`.
    async fn dump(&self, conv_id: &str) -> Result<ConversationDump>;

    /// Cancel a running conversation.
    async fn cancel(&self, conv_id: &str) -> Result<()>;

    /// Attach a mailbox so the engine can emit/consume A2A messages.
    async fn wire_mailbox(&self, conv_id: &str, mailbox: &Mailbox) -> Result<()>;

    /// Normalize a raw dump into a [`StructuredResult`]. Pure transform;
    /// implementations must not perform IO here.
    fn extract_result(&self, dump: &ConversationDump) -> Result<StructuredResult>;

    /// Advertise static capabilities.
    fn capabilities(&self) -> EngineCapabilities;
}

/// Selects an engine/agent target for a task.
///
/// Adapters may delegate to [`crate::routing_port::RoutingSuperset`] for
/// load-balancing strategies, per-target circuit breakers, and weighted fallback.
#[async_trait]
pub trait RoutingPort: Send + Sync {
    /// Return the engine name that should handle `task`.
    async fn route(&self, task: &Task) -> Result<String> {
        // Backwards-compatible default: delegate to the structured decision.
        Ok(self.route_decision(task).await?.engine)
    }

    /// Return a full [`RoutingDecision`] (engine + model + rationale).
    ///
    /// Phase 1 introduces the structured decision so adapters (e.g. the
    /// `routing-phenotype-router` adapter) can route to a specific
    /// model/provider while preserving the engine target.
    async fn route_decision(&self, task: &Task) -> Result<RoutingDecision>;
}

/// A message bus / mailbox transport.
///
/// # Atomic claim contract
///
/// [`claim`](TransportPort::claim) MUST be atomic: at most one caller may
/// successfully claim a given message. Implementations realize this via a
/// compare-and-swap on a `state`/lease field (e.g. a lockfile + CAS), so that
/// concurrent workers never double-process. A failed claim returns
/// [`crate::error::SubstrateError::ClaimConflict`].
#[async_trait]
pub trait TransportPort: Send + Sync {
    /// Publish `message` to its recipient's mailbox.
    async fn publish(&self, message: &Message) -> Result<()>;

    /// Return all messages currently addressed to `owner`.
    async fn subscribe(&self, owner: &str) -> Result<Vec<Message>>;

    /// Atomically claim `message_id` for exclusive processing (CAS-lease).
    /// Returns the claimed message, or `ClaimConflict` if already held.
    async fn claim(&self, owner: &str, message_id: &uuid::Uuid) -> Result<Message>;

    /// Snapshot the full mailbox for `owner`.
    async fn mailbox(&self, owner: &str) -> Result<Mailbox>;
}

/// Durable persistence for tasks and results.
///
/// [`claim_atomic`](StorePort::claim_atomic) provides the same CAS-lease
/// guarantee as the transport: at most one worker may move a task out of its
/// claimable state.
#[async_trait]
pub trait StorePort: Send + Sync {
    /// Persist (insert or update) a task.
    async fn persist(&self, task: &Task) -> Result<()>;

    /// Load a task by id.
    async fn load(&self, id: &uuid::Uuid) -> Result<Task>;

    /// Persist a normalized result for a task.
    async fn persist_result(&self, task_id: &uuid::Uuid, result: &StructuredResult) -> Result<()>;

    /// Atomically claim a task for exclusive work (CAS on lifecycle state).
    /// Returns `ClaimConflict` if another worker already holds it.
    async fn claim_atomic(&self, id: &uuid::Uuid) -> Result<Task>;
}

/// The inbound application API (driving side of the hexagon).
#[async_trait]
pub trait DispatchApi: Send + Sync {
    /// Dispatch a task end-to-end and return its normalized result.
    async fn dispatch(&self, task: Task) -> Result<StructuredResult>;

    /// Fetch a previously dispatched task.
    async fn get(&self, id: &uuid::Uuid) -> Result<Task>;

    /// Cancel a previously dispatched task.
    async fn cancel(&self, id: &uuid::Uuid) -> Result<()>;
}
