//! Pure domain model: entities, value objects, and the task lifecycle FSM.
//!
//! Nothing in this module performs IO. Every type is serde-serializable so
//! that adapters can persist/transport them without re-declaring shapes.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, SubstrateError};

// ---------------------------------------------------------------------------
// Task lifecycle
// ---------------------------------------------------------------------------

/// The lifecycle state of a [`Task`].
///
/// Legal flow:
/// `Submitted -> Working -> InputRequired -> Working -> Completed`
/// with `Failed`/`Cancelled` reachable from any non-terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Accepted into the system, not yet picked up.
    Submitted,
    /// Actively being worked by an engine.
    Working,
    /// Blocked awaiting human/agent input.
    InputRequired,
    /// Finished successfully (terminal).
    Completed,
    /// Finished unsuccessfully (terminal).
    Failed,
    /// Aborted before completion (terminal).
    Cancelled,
}

impl TaskState {
    /// Returns true if no further transitions are legal from this state.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Pure transition predicate for the lifecycle FSM.
    ///
    /// Terminal states have no outgoing edges. Non-terminal states may always
    /// move to `Failed` or `Cancelled`. The happy-path edges are explicit.
    pub fn can_transition(from: TaskState, to: TaskState) -> bool {
        use TaskState::*;
        if from == to {
            return false;
        }
        if from.is_terminal() {
            return false;
        }
        // Any live task may fail or be cancelled.
        if matches!(to, Failed | Cancelled) {
            return true;
        }
        matches!(
            (from, to),
            (Submitted, Working)
                | (Working, InputRequired)
                | (Working, Completed)
                | (InputRequired, Working)
        )
    }
}

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

/// A unit of dispatchable work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Stable identity.
    pub id: Uuid,
    /// The instruction handed to an engine.
    pub prompt: String,
    /// Working directory the engine should run in.
    pub cwd: String,
    /// Current lifecycle state.
    pub state: TaskState,
    /// Parent task, if this is a delegated subtask.
    pub parent_task_id: Option<Uuid>,
    /// Traceability link to a requirement (FR/NFR id).
    pub requirement_id: Option<String>,
    /// Traceability link to an epic.
    pub epic_id: Option<String>,
    /// Engine conversation id, set after [`EnginePort::start`].
    #[serde(default)]
    pub conv_id: Option<String>,
}

impl Task {
    /// Create a freshly-submitted task with a new id.
    pub fn new(prompt: impl Into<String>, cwd: impl Into<String>) -> Self {
        Task {
            id: Uuid::new_v4(),
            prompt: prompt.into(),
            cwd: cwd.into(),
            state: TaskState::Submitted,
            parent_task_id: None,
            requirement_id: None,
            epic_id: None,
            conv_id: None,
        }
    }

    /// Advance the task to `to`, enforcing the lifecycle FSM.
    ///
    /// Returns [`SubstrateError::InvalidTransition`] if the edge is illegal,
    /// leaving the task unchanged.
    pub fn advance(&mut self, to: TaskState) -> Result<()> {
        if TaskState::can_transition(self.state, to) {
            self.state = to;
            Ok(())
        } else {
            Err(SubstrateError::InvalidTransition {
                from: self.state,
                to,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Agents & teams
// ---------------------------------------------------------------------------

/// The role an agent plays in a team hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Owns a stream end-to-end and delegates downward.
    Lead,
    /// A peer working under a lead.
    Teammate,
    /// A worker spawned by a lead/teammate.
    Subagent,
}

/// An addressable participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    /// Stable identity.
    pub id: Uuid,
    /// Human-readable handle (addressable in a mailbox).
    pub name: String,
    /// Role in the hierarchy.
    pub role: AgentRole,
    /// The engine that backs this agent (e.g. "forge").
    pub engine: String,
}

/// A named set of agents collaborating on a goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    /// Stable identity.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// The lead agent's id.
    pub lead: Uuid,
    /// All member ids (including the lead).
    pub members: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// A2A-shaped messaging
// ---------------------------------------------------------------------------

/// The semantic kind of a [`Message`], mirroring the A2A protocol shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// A delegated unit of work.
    Task,
    /// A response to a prior message.
    Reply,
    /// A request for information/decision.
    Question,
    /// A progress/status update.
    Status,
    /// An emitted artifact (file, PR, diff).
    Artifact,
}

/// A single content part of a message (A2A `parts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// Plain text content.
    Text {
        /// The text body.
        text: String,
    },
    /// A reference to a produced artifact.
    Artifact {
        /// Artifact name/identifier.
        name: String,
        /// Where the artifact lives (path/URL).
        uri: String,
    },
}

/// An A2A-shaped message exchanged between agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Stable identity.
    pub id: Uuid,
    /// Sender handle.
    pub from: String,
    /// Recipient handle.
    pub to: String,
    /// Semantic kind.
    pub kind: MessageKind,
    /// Ordered content parts.
    pub parts: Vec<Part>,
    /// The message this is a reply to, if any.
    pub in_reply_to: Option<Uuid>,
}

/// A per-recipient ordered collection of messages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mailbox {
    /// Owner handle of this mailbox.
    pub owner: String,
    /// Messages in arrival order.
    pub messages: Vec<Message>,
}

/// A threaded exchange of messages, scoped to an engine conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    /// Stable identity (often the engine's conversation id).
    pub id: String,
    /// Ordered messages in the thread.
    pub messages: Vec<Message>,
}

// ---------------------------------------------------------------------------
// Engine I/O value objects
// ---------------------------------------------------------------------------

/// The normalized outcome of an engine run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredResult {
    /// Final assistant text.
    pub text: String,
    /// Emitted artifacts (name -> uri).
    pub artifacts: Vec<Part>,
    /// Pull-request URLs discovered in the output.
    pub pr_urls: Vec<String>,
    /// Terminal lifecycle status implied by the run.
    pub status: TaskState,
}

/// Static capabilities advertised by an engine adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCapabilities {
    /// Can resume an existing conversation by id.
    pub supports_resume: bool,
    /// Can spawn subagents.
    pub supports_subagents: bool,
    /// Can import MCP server definitions.
    pub supports_mcp_import: bool,
}

/// A raw, engine-specific conversation export (pre-normalization).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationDump {
    /// The engine conversation id this dump came from.
    pub conversation_id: String,
    /// The raw payload (usually JSON text from the engine).
    pub raw: String,
}

/// A live (or recorded) engine process handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The engine conversation id.
    pub conv_id: String,
    /// OS process id, if the engine ran as a subprocess.
    pub pid: Option<u32>,
    /// Path to the captured log file, if any.
    pub logfile: Option<String>,
}

/// A routing decision: which engine + which model the router chose for a task.
///
/// Returned by [`crate::ports::RoutingPort::route_decision`]. The default
/// engine is `forge`; the default model is
/// `accounts/fireworks/routers/kimi-k2p6-turbo` (Phase 1's OmniRoute target).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// The engine name that should handle the task (e.g. `"forge"`).
    pub engine: String,
    /// The model identifier the engine should use.
    pub model: String,
    /// Free-form rationale (provider hint, reason code, etc.).
    #[serde(default)]
    pub reason: Option<String>,
}

impl RoutingDecision {
    /// The Phase 1 default decision (forge + OmniRoute kimi router).
    pub fn default_forge_kimi() -> Self {
        RoutingDecision {
            engine: "forge".to_string(),
            model: "accounts/fireworks/routers/kimi-k2p6-turbo".to_string(),
            reason: Some("phase1-default".to_string()),
        }
    }
}
