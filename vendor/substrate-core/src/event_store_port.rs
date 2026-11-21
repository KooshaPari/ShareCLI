//! EventStorePort — append-only per-aggregate event log with global ordering.
//!
//! Core defines the contract and replay/projection helpers; `store-sqlite`
//! provides the durable SQLite implementation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::TaskState;

/// A persisted domain event with monotonic per-aggregate and global sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope<E> {
    /// Aggregate this event belongs to.
    pub aggregate_id: Uuid,
    /// Monotonic sequence within the aggregate (0-based).
    pub aggregate_seq: u64,
    /// Monotonic sequence across all aggregates.
    pub global_seq: u64,
    /// Domain event payload.
    pub event: E,
    /// Unix epoch seconds when the event was recorded.
    pub occurred_at: i64,
}

/// Append-only event log port.
///
/// Implementations MUST guarantee:
/// - events for an aggregate are returned in `aggregate_seq` order;
/// - `global_seq` is strictly monotonic across all appends;
/// - duplicate `(aggregate_id, aggregate_seq)` appends are rejected.
pub trait EventStorePort: Send + Sync {
    /// Error type returned by event-store operations.
    type Error: std::error::Error + Send + Sync + 'static;
    /// Serde-serializable domain event type stored by this port.
    type Event: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync;

    /// Append `event` to the log for `aggregate_id`.
    ///
    /// `expected_seq` is the sequence number this event will receive (equal to
    /// the current event count for the aggregate). Mismatch rejects the append.
    fn append(
        &self,
        aggregate_id: Uuid,
        expected_seq: u64,
        event: &Self::Event,
    ) -> Result<EventEnvelope<Self::Event>, Self::Error>;

    /// Load all events for `aggregate_id` in `aggregate_seq` order.
    fn load(&self, aggregate_id: Uuid) -> Result<Vec<EventEnvelope<Self::Event>>, Self::Error>;
}

/// Fold an ordered event stream into aggregate state.
pub trait Projection {
    /// Rebuilt aggregate state.
    type State;
    /// Domain event type consumed by this projection.
    type Event;

    /// State before any events are applied.
    fn initial() -> Self::State;

    /// Apply a single event to `state`, returning the new state.
    fn apply(state: Self::State, event: &Self::Event) -> Self::State;
}

/// Replay `events` through `P`, returning the folded state.
pub fn replay<P: Projection>(events: &[P::Event]) -> P::State {
    events
        .iter()
        .fold(P::initial(), |state, event| P::apply(state, event))
}

// ---------------------------------------------------------------------------
// Task lifecycle demonstration projection
// ---------------------------------------------------------------------------

/// Task lifecycle events for event-sourced replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskLifecycleEvent {
    /// Task created with prompt and working directory.
    Created {
        /// Instruction handed to an engine.
        prompt: String,
        /// Working directory.
        cwd: String,
    },
    /// Lifecycle transition enforced by the FSM at replay time.
    Advanced {
        /// Target lifecycle state.
        to: TaskState,
    },
}

/// State rebuilt from a [`TaskLifecycleEvent`] stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProjectionState {
    /// Aggregate identity (set on first `Created`).
    pub id: Option<Uuid>,
    /// Task prompt.
    pub prompt: String,
    /// Working directory.
    pub cwd: String,
    /// Current lifecycle state.
    pub state: TaskState,
}

/// Projection that folds [`TaskLifecycleEvent`] into [`TaskProjectionState`].
pub struct TaskLifecycleProjection;

impl Projection for TaskLifecycleProjection {
    type State = TaskProjectionState;
    type Event = TaskLifecycleEvent;

    fn initial() -> Self::State {
        TaskProjectionState {
            id: None,
            prompt: String::new(),
            cwd: String::new(),
            state: TaskState::Submitted,
        }
    }

    fn apply(mut state: Self::State, event: &Self::Event) -> Self::State {
        match event {
            TaskLifecycleEvent::Created { prompt, cwd } => {
                state.prompt = prompt.clone();
                state.cwd = cwd.clone();
                state.state = TaskState::Submitted;
            },
            TaskLifecycleEvent::Advanced { to } => {
                if TaskState::can_transition(state.state, *to) {
                    state.state = *to;
                }
            },
        }
        state
    }
}

/// Replay task lifecycle events for `aggregate_id` from `store`.
pub fn replay_task_state<S>(store: &S, aggregate_id: Uuid) -> Result<TaskProjectionState, S::Error>
where
    S: EventStorePort<Event = TaskLifecycleEvent>,
{
    let envelopes = store.load(aggregate_id)?;
    let events: Vec<TaskLifecycleEvent> = envelopes.into_iter().map(|e| e.event).collect();
    Ok(replay::<TaskLifecycleProjection>(&events))
}
