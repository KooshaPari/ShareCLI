//! # engine-spec
//!
//! Provider-agnostic translation of a [`TaskSpec`] into a process `argv`.
//! Engine adapters implement [`ArgvBuilder`] to map the neutral spec onto
//! their own CLI surface.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// A neutral description of what an engine should run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// The prompt/instruction.
    pub prompt: String,
    /// Working directory.
    pub cwd: String,
    /// Named agent/persona to invoke, if the engine supports it.
    pub agent: Option<String>,
    /// Conversation id to resume, if resuming.
    pub resume: Option<String>,
}

impl TaskSpec {
    /// Build a fresh spec for a new run.
    pub fn new(prompt: impl Into<String>, cwd: impl Into<String>) -> Self {
        TaskSpec {
            prompt: prompt.into(),
            cwd: cwd.into(),
            agent: None,
            resume: None,
        }
    }

    /// Set the named agent.
    pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }
}

/// Translates a [`TaskSpec`] into a concrete process `argv`.
///
/// The returned vector is `[program, arg0, arg1, ...]`; element 0 is the
/// program/binary to execute.
pub trait ArgvBuilder {
    /// Produce the argv for starting a new run.
    fn build_start(&self, spec: &TaskSpec) -> Vec<String>;

    /// Produce the argv for dumping a conversation by id.
    fn build_dump(&self, conversation_id: &str) -> Vec<String>;
}
