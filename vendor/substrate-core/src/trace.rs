//! Cross-cutting trace port: the trait lives in core so the application layer
//! can emit events without knowing which backend receives them.
//!
//! **Rule:** only the trait and the event structs live here. Adapter
//! implementations (`NoopTrace`, `RecordingTrace`, `MultiTrace`, etc.) belong
//! in the `substrate-trace` crate.

/// An event fired when a task is first registered with the system.
#[derive(Debug, Clone)]
pub struct TaskRegistered {
    /// Stable task identity.
    pub task_id: String,
    /// Traceability link to a requirement (FR/NFR id), if present.
    pub requirement_id: Option<String>,
    /// Traceability link to an epic, if present.
    pub epic_id: Option<String>,
}

/// An event fired when a task completes successfully.
#[derive(Debug, Clone)]
pub struct TaskCompleted {
    /// Stable task identity.
    pub task_id: String,
    /// Pull-request URLs emitted by the run.
    pub pr_urls: Vec<String>,
    /// Traceability link to the originating requirement, if present.
    pub requirement_id: Option<String>,
}

/// An event fired when a task fails.
#[derive(Debug, Clone)]
pub struct TaskFailed {
    /// Stable task identity.
    pub task_id: String,
    /// Human-readable failure description.
    pub error: String,
    /// Traceability link to the originating requirement, if present.
    pub requirement_id: Option<String>,
}

/// The cross-cutting trace port.
///
/// Adapters implement this to ship events to AgilePlus, Tracera, an in-memory
/// recording double, or a fan-out multiplexer. The port is defined here in
/// core so that `DispatchService` can emit events without depending on any
/// adapter crate.
pub trait TracePort: Send + Sync {
    /// Called once when a task is accepted into the system.
    fn task_registered(&self, event: TaskRegistered);

    /// Called once when a task run concludes successfully.
    fn task_completed(&self, event: TaskCompleted);

    /// Called once when a task run concludes with a failure.
    fn task_failed(&self, event: TaskFailed);
}
