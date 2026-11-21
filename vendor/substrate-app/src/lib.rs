//! # substrate-app
//!
//! The application/use-case layer. [`DispatchService`] orchestrates the
//! Phase-0 dispatch flow purely against the core ports — it knows nothing
//! about files, processes, or the forge CLI. The composition root
//! (`driver-cli`) supplies concrete adapters.
//!
//! Phase 5 adds optional [`TracePort`] emission: when a [`TracePort`] is
//! wired in, `dispatch()` fires `TaskRegistered` at the start and either
//! `TaskCompleted` or `TaskFailed` at the end.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod dispatch_planner;
pub mod tiered_dispatch;

use std::sync::Arc;

use async_trait::async_trait;
pub use dispatch_planner::{
    DispatchPlan, DispatchPlanner, EngineCandidate, PlanRequest, SessionMode,
};
use substrate_core::{
    domain::{StructuredResult, Task, TaskState},
    error::{Result, SubstrateError},
    ports::{DispatchApi, EnginePort, StorePort, TransportPort},
    trace::{TaskCompleted, TaskFailed, TaskRegistered, TracePort},
};
use uuid::Uuid;

/// Orchestrates dispatch over the three driven ports.
///
/// Generic over the concrete [`EnginePort`], [`StorePort`], and
/// [`TransportPort`] implementations so the use-case is testable with fakes
/// and reusable across adapters.
///
/// An optional [`TracePort`] can be wired in via
/// [`DispatchService::with_trace`] to emit lifecycle events to AgilePlus,
/// Tracera, or any other backend.
pub struct DispatchService<E, S, T> {
    engine: Arc<E>,
    store: Arc<S>,
    #[allow(dead_code)]
    transport: Arc<T>,
    trace: Option<Arc<dyn TracePort>>,
}

impl<E, S, T> DispatchService<E, S, T>
where
    E: EnginePort,
    S: StorePort,
    T: TransportPort,
{
    /// Wire the service from its ports (no trace backend).
    pub fn new(engine: Arc<E>, store: Arc<S>, transport: Arc<T>) -> Self {
        DispatchService {
            engine,
            store,
            transport,
            trace: None,
        }
    }

    /// Attach a [`TracePort`] for lifecycle event emission.
    pub fn with_trace(mut self, trace: Arc<dyn TracePort>) -> Self {
        self.trace = Some(trace);
        self
    }

    /// Helper: emit a `TaskRegistered` event if a trace is attached.
    fn emit_registered(&self, task: &Task) {
        if let Some(t) = &self.trace {
            t.task_registered(TaskRegistered {
                task_id: task.id.to_string(),
                requirement_id: task.requirement_id.clone(),
                epic_id: task.epic_id.clone(),
            });
        }
    }

    /// Helper: emit a `TaskCompleted` event if a trace is attached.
    fn emit_completed(&self, task: &Task, result: &StructuredResult) {
        if let Some(t) = &self.trace {
            t.task_completed(TaskCompleted {
                task_id: task.id.to_string(),
                pr_urls: result.pr_urls.clone(),
                requirement_id: task.requirement_id.clone(),
            });
        }
    }

    /// Helper: emit a `TaskFailed` event if a trace is attached.
    fn emit_failed(&self, task: &Task, error: &str) {
        if let Some(t) = &self.trace {
            t.task_failed(TaskFailed {
                task_id: task.id.to_string(),
                error: error.to_string(),
                requirement_id: task.requirement_id.clone(),
            });
        }
    }
}

#[async_trait]
impl<E, S, T> DispatchApi for DispatchService<E, S, T>
where
    E: EnginePort,
    S: StorePort,
    T: TransportPort,
{
    async fn dispatch(&self, mut task: Task) -> Result<StructuredResult> {
        // 1. Persist the submitted task.
        self.store.persist(&task).await?;

        // 2. Emit TaskRegistered.
        self.emit_registered(&task);

        // 3. Move to Working and start the engine.
        task.advance(TaskState::Working)?;
        self.store.persist(&task).await?;
        let session = match self.engine.start(&task).await {
            Ok(s) => s,
            Err(e) => {
                self.emit_failed(&task, &e.to_string());
                return Err(e);
            },
        };
        task.conv_id = Some(session.conv_id.clone());
        self.store.persist(&task).await?;

        // 4. Dump and normalize.
        let dump = match self.engine.dump(&session.conv_id).await {
            Ok(d) => d,
            Err(e) => {
                self.emit_failed(&task, &e.to_string());
                return Err(e);
            },
        };
        let result = match self.engine.extract_result(&dump) {
            Ok(r) => r,
            Err(e) => {
                self.emit_failed(&task, &e.to_string());
                return Err(e);
            },
        };

        // 5. Reflect the engine's terminal status onto the task.
        if task.state != result.status {
            task.advance(result.status)?;
        }
        self.store.persist(&task).await?;
        self.store.persist_result(&task.id, &result).await?;

        // 6. Emit TaskCompleted or TaskFailed only for terminal engine status.
        match result.status {
            TaskState::Failed => self.emit_failed(&task, &result.text),
            TaskState::Completed => self.emit_completed(&task, &result),
            _ => {},
        }

        Ok(result)
    }

    async fn get(&self, id: &Uuid) -> Result<Task> {
        self.store.load(id).await
    }

    async fn cancel(&self, id: &Uuid) -> Result<()> {
        let mut task = self.store.load(id).await?;
        if task.state.is_terminal() {
            return Err(SubstrateError::InvalidTransition {
                from: task.state,
                to: TaskState::Cancelled,
            });
        }
        let conv_id = task.conv_id.as_deref().ok_or_else(|| {
            SubstrateError::Engine(format!("task {id} has no engine session to cancel"))
        })?;
        self.engine.cancel(conv_id).await?;
        task.advance(TaskState::Cancelled)?;
        self.store.persist(&task).await?;
        Ok(())
    }
}
