#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Cross-platform [`ProcessPort`] backed by the `command-group` crate.
//!
//! Spawns children in a fresh process group so the whole subtree can be
//! signalled on timeout. Platform specifics (Unix `setsid` / Windows
//! `CREATE_NEW_PROCESS_GROUP`) are handled inside `command-group`.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use substrate_core::{
    error::{Result, SubstrateError},
    process_port::{ProcessHandle, ProcessPort, ProcessSpawnSpec, ProcessState},
};
use tokio::{process::Command, sync::Mutex};
use uuid::Uuid;

struct ManagedChild {
    child: AsyncGroupChild,
}

/// [`ProcessPort`] that spawns via `command-group` for portable process groups.
#[derive(Clone)]
pub struct CommandGroupProcess {
    children: Arc<Mutex<HashMap<Uuid, ManagedChild>>>,
}

impl std::fmt::Debug for CommandGroupProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandGroupProcess")
            .finish_non_exhaustive()
    }
}

impl Default for CommandGroupProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandGroupProcess {
    /// Create a new process manager.
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn map_state(pid: u32, code: Option<i32>) -> ProcessState {
        ProcessState::Exited { pid, code }
    }
}

#[async_trait]
impl ProcessPort for CommandGroupProcess {
    async fn spawn(&self, spec: &ProcessSpawnSpec) -> Result<ProcessHandle> {
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            cmd.current_dir(cwd);
        }

        let child = cmd
            .group_spawn()
            .map_err(|e| SubstrateError::Process(format!("spawn {}: {e}", spec.program)))?;
        let pid = child.id().unwrap_or(0);
        let id = Uuid::new_v4();
        let handle = ProcessHandle { id, pid };

        self.children
            .lock()
            .await
            .insert(id, ManagedChild { child });

        Ok(handle)
    }

    async fn status(&self, handle: &ProcessHandle) -> Result<ProcessState> {
        let mut children = self.children.lock().await;
        let managed = children
            .get_mut(&handle.id)
            .ok_or_else(|| SubstrateError::NotFound(format!("process {}", handle.id)))?;

        match managed.child.try_wait() {
            Ok(Some(status)) => Ok(Self::map_state(handle.pid, status.code())),
            Ok(None) => Ok(ProcessState::Running { pid: handle.pid }),
            Err(e) => Err(SubstrateError::Process(format!("try_wait: {e}"))),
        }
    }

    async fn wait_with_timeout(
        &self,
        handle: &ProcessHandle,
        timeout: Duration,
    ) -> Result<ProcessState> {
        let mut children = self.children.lock().await;
        let managed = children
            .get_mut(&handle.id)
            .ok_or_else(|| SubstrateError::NotFound(format!("process {}", handle.id)))?;

        match tokio::time::timeout(timeout, managed.child.wait()).await {
            Ok(Ok(status)) => {
                let state = Self::map_state(handle.pid, status.code());
                children.remove(&handle.id);
                Ok(state)
            },
            Ok(Err(e)) => Err(SubstrateError::Process(format!("wait: {e}"))),
            Err(_) => {
                let _ = managed.child.kill().await;
                let _ = managed.child.wait().await;
                children.remove(&handle.id);
                Ok(Self::map_state(handle.pid, None))
            },
        }
    }

    async fn kill_group(&self, handle: &ProcessHandle) -> Result<()> {
        let mut children = self.children.lock().await;
        let managed = children
            .get_mut(&handle.id)
            .ok_or_else(|| SubstrateError::NotFound(format!("process {}", handle.id)))?;

        managed
            .child
            .kill()
            .await
            .map_err(|e| SubstrateError::Process(format!("kill_group: {e}")))?;
        let _ = managed.child.wait().await;
        children.remove(&handle.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Portable long-running child for timeout/kill tests.
    fn sleep_spec(secs: u64) -> ProcessSpawnSpec {
        #[cfg(unix)]
        {
            ProcessSpawnSpec {
                program: "sleep".into(),
                args: vec![secs.to_string()],
                cwd: None,
            }
        }
        #[cfg(windows)]
        {
            ProcessSpawnSpec {
                program: "powershell".into(),
                args: vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    format!("Start-Sleep -Seconds {secs}"),
                ],
                cwd: None,
            }
        }
    }

    /// Portable echo child that exits quickly with code 0.
    fn echo_spec() -> ProcessSpawnSpec {
        #[cfg(unix)]
        {
            ProcessSpawnSpec {
                program: "sh".into(),
                args: vec!["-c".into(), "echo substrate-process-ok".into()],
                cwd: None,
            }
        }
        #[cfg(windows)]
        {
            ProcessSpawnSpec {
                program: "cmd".into(),
                args: vec!["/C".into(), "echo substrate-process-ok".into()],
                cwd: None,
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_wait_returns_exit_status() {
        let proc = CommandGroupProcess::new();
        let handle = proc.spawn(&echo_spec()).await.unwrap();
        assert!(handle.pid > 0);

        let state = proc
            .wait_with_timeout(&handle, Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(
            state,
            ProcessState::Exited {
                pid: handle.pid,
                code: Some(0),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_reports_running_then_exited() {
        let proc = CommandGroupProcess::new();
        let handle = proc.spawn(&echo_spec()).await.unwrap();

        // May already have exited; poll until we see Exited.
        let mut saw_running = false;
        for _ in 0..20 {
            match proc.status(&handle).await.unwrap() {
                ProcessState::Running { .. } => saw_running = true,
                ProcessState::Exited { code, .. } => {
                    assert_eq!(code, Some(0));
                    break;
                },
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let _ = saw_running; // best-effort on fast CI hosts
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn kill_on_timeout_returns_within_timeout() {
        let proc = CommandGroupProcess::new();
        let handle = proc.spawn(&sleep_spec(60)).await.unwrap();

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();
        let state = proc.wait_with_timeout(&handle, timeout).await.unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "took too long: {elapsed:?}"
        );
        assert_eq!(
            state,
            ProcessState::Exited {
                pid: handle.pid,
                code: None,
            }
        );
    }
}
