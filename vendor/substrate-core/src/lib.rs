//! # substrate-core
//!
//! The hexagonal **core**: pure domain contracts and the port traits that
//! adapters implement. This crate depends on nothing but `serde`,
//! `serde_json`, `thiserror`, `uuid`, and `async-trait` (required to express
//! async port traits). It MUST NOT depend on any adapter crate
//! (`engine-*`, `transport-*`, `store-*`, `driver-*`, `*-adapter`); this is
//! enforced by `crates/arch-test`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod claim_port;
pub mod cloud_dispatch_port;
pub mod domain;
pub mod error;
pub mod event_store_port;
pub mod mailbox_port;
pub mod memory_port;
pub mod ports;
pub mod process_port;
pub mod routing_port;
pub mod schedule_port;
pub mod skill_port;
pub mod tiers;
pub mod trace;
pub mod watcher_port;
pub mod workflow_port;

pub use claim_port::{ClaimPort, WorkItem, WorkItemState};
pub use cloud_dispatch_port::{CloudDispatchPort, CloudResult, CloudTaskHandle, CloudTaskStatus};
pub use error::{Result, SubstrateError};
pub use event_store_port::{
    replay, replay_task_state, EventEnvelope, EventStorePort, Projection, TaskLifecycleEvent,
    TaskLifecycleProjection, TaskProjectionState,
};
pub use mailbox_port::MailboxStore;
pub use memory_port::{MemoryEntry, MemoryPort};
pub use process_port::{ProcessHandle, ProcessPort, ProcessSpawnSpec, ProcessState};
pub use routing_port::{
    CircuitBreaker, CircuitBreakerConfig, CircuitState, FallbackEntry, RoutingPoolState,
    RoutingSelector, RoutingStrategy, RoutingSuperset, RoutingTarget, SupersetRoutingDecision,
    TargetHealth,
};
pub use schedule_port::{ScheduleInstant, SchedulePort, ScheduleTrigger, Weekday};
pub use skill_port::{
    validate_json_schema, SkillDescriptor, SkillHandler, SkillPort, ToolRegistry,
};
pub use tiers::{
    Tier, TierSpec, HEAVY_MODEL, HEAVY_REASONING_EFFORT, MAIN_MODEL, MAIN_REASONING_EFFORT,
    WORKER_MODEL, WORKER_REASONING_EFFORT,
};
pub use trace::{TaskCompleted, TaskFailed, TaskRegistered, TracePort};
pub use watcher_port::{WatchEvent, WatchEventKind, WatchHandle, WatcherPort};
pub use workflow_port::{Workflow, WorkflowEdge, WorkflowNode, WorkflowPort};
