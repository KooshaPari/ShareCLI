use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The semantic category of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// A work assignment.
    Task,
    /// A reply to a previous message.
    Reply,
    /// A question requiring a response.
    Question,
    /// A status update.
    Status,
    /// A file or data artifact.
    Artifact,
}

/// A single content item within a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// Plain text content.
    Text {
        /// The text payload.
        text: String,
    },
    /// Structured JSON data.
    Data {
        /// The JSON payload.
        data: serde_json::Value,
    },
    /// A file reference by URI.
    File {
        /// The file URI.
        uri: String,
    },
}

/// Message delivery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgState {
    /// Not yet claimed by any worker.
    Unread,
    /// Claimed by exactly one worker (atomic).
    Delivered,
    /// Fully processed.
    Consumed,
}

/// An agent-to-agent message in the team mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: Uuid,
    /// Team this message belongs to.
    pub team_id: String,
    /// Optional associated task.
    pub task_id: Option<Uuid>,
    /// Sending agent name.
    pub from: String,
    /// Receiving agent name.
    pub to: String,
    /// Message category.
    pub kind: MessageKind,
    /// Message content parts.
    pub parts: Vec<Part>,
    /// Optional reference to the message being replied to.
    pub in_reply_to: Option<Uuid>,
    /// Delivery state.
    pub state: MsgState,
    /// When the message was created.
    pub created_at: DateTime<Utc>,
    /// When the message was consumed, if at all.
    pub consumed_at: Option<DateTime<Utc>>,
}

impl Message {
    /// Create a new unread message.
    pub fn new(
        team_id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        kind: MessageKind,
        parts: Vec<Part>,
    ) -> Self {
        Message {
            id: Uuid::new_v4(),
            team_id: team_id.into(),
            task_id: None,
            from: from.into(),
            to: to.into(),
            kind,
            parts,
            in_reply_to: None,
            state: MsgState::Unread,
            created_at: Utc::now(),
            consumed_at: None,
        }
    }
}

/// A file or data artifact produced by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// The artifact category (e.g. "diff", "report").
    pub kind: String,
    /// The artifact content (text, base64, or URI).
    pub content: String,
    /// Optional human-readable name.
    pub name: Option<String>,
}
