use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Task lifecycle state (A2A variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Submitted but not yet picked up by a worker.
    Submitted,
    /// Actively being worked on.
    Working,
    /// Paused, waiting for caller input.
    InputRequired,
    /// Work finished successfully.
    Completed,
    /// Work ended in an error.
    Failed,
    /// Explicitly cancelled.
    Cancelled,
}

impl TaskState {
    /// Returns true if no further transitions are legal.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Pure transition predicate — does not mutate state.
    pub fn can_transition(from: TaskState, to: TaskState) -> bool {
        use TaskState::*;
        if from == to {
            return false;
        }
        if from.is_terminal() {
            return false;
        }
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

/// A unit of team-scoped work with traceability links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: Uuid,
    /// Team this task belongs to.
    pub team_id: String,
    /// Human-readable title.
    pub title: String,
    /// Current lifecycle state.
    pub state: TaskState,
    /// Agent name that owns this task.
    pub owner: String,
    /// Optional parent task for hierarchical decomposition.
    pub parent_task_id: Option<Uuid>,
    /// Optional traceability link to a requirements system.
    pub requirement_id: Option<String>,
    /// Optional epic identifier.
    pub epic_id: Option<String>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Create a new task in `Submitted` state.
    pub fn new(
        team_id: impl Into<String>,
        title: impl Into<String>,
        owner: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Task {
            id: Uuid::new_v4(),
            team_id: team_id.into(),
            title: title.into(),
            state: TaskState::Submitted,
            owner: owner.into(),
            parent_task_id: None,
            requirement_id: None,
            epic_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}
