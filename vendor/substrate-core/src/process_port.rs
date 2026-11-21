//! ProcessPort — cross-platform managed subprocess spawn/monitor/kill.
//!
//! Core defines the port contract and value types; `runtime-process` wraps a
//! vetted process-group crate for platform-specific group semantics.

use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

/// Specification for spawning a managed child process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessSpawnSpec {
    /// Executable name or path.
    pub program: String,
    /// Arguments passed to the program.
    pub args: Vec<String>,
    /// Optional working directory for the child.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
}

/// Opaque handle to a managed child returned by [`ProcessPort::spawn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessHandle {
    /// Adapter-local process id.
    pub id: Uuid,
    /// OS process (or process-group leader) pid at spawn time.
    pub pid: u32,
}

/// Observed lifecycle state of a managed child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    /// Child is still running.
    Running {
        /// Observed pid.
        pid: u32,
    },
    /// Child has exited (naturally or after a kill).
    Exited {
        /// Observed pid.
        pid: u32,
        /// Exit code when available (`None` after kill-on-timeout).
        code: Option<i32>,
    },
}

/// General managed-subprocess port: spawn in a process group, poll status,
/// wait with timeout (killing the group on expiry), and explicit group kill.
#[async_trait]
pub trait ProcessPort: Send + Sync {
    /// Spawn `spec` in its own process group and return a handle + pid.
    async fn spawn(&self, spec: &ProcessSpawnSpec) -> Result<ProcessHandle>;

    /// Poll the current state without blocking for exit.
    async fn status(&self, handle: &ProcessHandle) -> Result<ProcessState>;

    /// Wait up to `timeout` for exit. On timeout the whole process group is
    /// killed and [`ProcessState::Exited`] with `code: None` is returned.
    async fn wait_with_timeout(
        &self,
        handle: &ProcessHandle,
        timeout: Duration,
    ) -> Result<ProcessState>;

    /// Kill the managed process group immediately.
    async fn kill_group(&self, handle: &ProcessHandle) -> Result<()>;
}
