//! # substrate — Rust SDK
//!
//! Single dependency for repos that need dispatch planning, hexagonal ports,
//! and domain types without reimplementing substrate internals.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use substrate::{
//!     DispatchPlanner, EngineCandidate, EngineCapabilities, PlanRequest, SessionMode, TaskSpec,
//! };
//!
//! let spec = TaskSpec::new("fix the bug", "/repo");
//! let engines = vec![EngineCandidate {
//!     name: "forge".into(),
//!     capabilities: EngineCapabilities {
//!         supports_resume: true,
//!         supports_subagents: true,
//!         supports_mcp_import: false,
//!     },
//! }];
//! let plan = DispatchPlanner::plan(&PlanRequest {
//!     spec: &spec,
//!     engines: &engines,
//!     explicit_engine: Some("forge"),
//!     session_mode: Some(SessionMode::Foreground),
//!     routing_engine: Some("forge"),
//! })
//! .unwrap();
//! assert_eq!(plan.engine, "forge");
//! ```
//!
//! ## Features
//!
//! | Feature | Enables |
//! |---------|---------|
//! | `app` (default) | [`DispatchPlanner`], [`DispatchService`], planning types |
//! | `spec` (default) | [`TaskSpec`], [`ArgvBuilder`] |
//! | `a2a` | A2A wire-schema crate as [`a2a`] |
//!
//! Adapter crates (`store-sqlite` with bundled SQLite, `engine-forge`) are
//! separate workspace members — depend on them via git when needed:
//!
//! ```toml
//! store-sqlite = { git = "https://github.com/KooshaPari/substrate", package = "store-sqlite" }
//! engine-forge = { git = "https://github.com/KooshaPari/substrate", package = "engine-forge" }
//! ```
//!
//! ## Public surface
//!
//! - **Domain**: [`Task`], [`TaskState`], [`StructuredResult`], [`Session`], mailboxes, routing
//!   decisions
//! - **Ports** ([`ports`]): [`EnginePort`], [`StorePort`], [`TransportPort`], [`RoutingPort`],
//!   [`DispatchApi`], plus schedule/workflow/claim/skill/memory/process/watcher/event-store ports
//! - **Planning** (`app`): [`DispatchPlanner`], [`DispatchPlan`], [`PlanRequest`], [`SessionMode`]
//! - **Spec** (`spec`): [`TaskSpec`] for provider-agnostic argv building
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "spec")]
pub use engine_spec::{ArgvBuilder, TaskSpec};
#[cfg(feature = "app")]
pub use substrate_app::{
    DispatchPlan, DispatchPlanner, DispatchService, EngineCandidate, PlanRequest, SessionMode,
};
/// Domain entities and value objects (re-exported for ergonomic `use substrate::Task`).
pub use substrate_core::domain::{
    Agent, AgentRole, Conversation, ConversationDump, EngineCapabilities, Mailbox, Message,
    MessageKind, Part, RoutingDecision, Session, StructuredResult, Task, TaskState, Team,
};
pub use substrate_core::{
    claim_port, domain, error, event_store_port, mailbox_port, memory_port, ports,
    ports::{DispatchApi, EnginePort, RoutingPort, StorePort, TransportPort},
    process_port, replay, replay_task_state, routing_port, schedule_port, skill_port, trace,
    validate_json_schema, watcher_port, workflow_port, CircuitBreaker, CircuitBreakerConfig,
    CircuitState, ClaimPort, EventEnvelope, EventStorePort, FallbackEntry, MailboxStore,
    MemoryEntry, MemoryPort, ProcessHandle, ProcessPort, ProcessSpawnSpec, ProcessState,
    Projection, Result, RoutingPoolState, RoutingSelector, RoutingStrategy, RoutingSuperset,
    RoutingTarget, ScheduleInstant, SchedulePort, ScheduleTrigger, SkillDescriptor, SkillHandler,
    SkillPort, SubstrateError, SupersetRoutingDecision, TargetHealth, TaskCompleted, TaskFailed,
    TaskLifecycleEvent, TaskLifecycleProjection, TaskProjectionState, TaskRegistered, ToolRegistry,
    TracePort, WatchEvent, WatchEventKind, WatchHandle, WatcherPort, Weekday, WorkItem,
    WorkItemState, Workflow, WorkflowEdge, WorkflowNode, WorkflowPort,
};

/// A2A wire-schema types (distinct from [`domain`] task/message shapes).
#[cfg(feature = "a2a")]
pub mod a2a {
    pub use ::a2a::*;
}
