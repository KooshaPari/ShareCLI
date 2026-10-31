// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `SharedRuntime` — node / bun process pools used by web harness commands.
//!
//! The pool is separate from `ProcessPool`: `ProcessPool` is a generic
//! spawn-and-track store (one per project, with optional `SpawnPolicy`
//! throttling for build harnesses), whereas `SharedRuntime` is a pool of
//! long-lived node / bun child processes that the runtime can `acquire` /
//! `release` so multiple commands can share a single warm process.

use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use sysinfo::Pid;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::config;

use super::health::PoolStatus;
use super::health::RuntimeHealth;

/// Shared runtime pool for node / bun web harness processes. Acquire a
/// pooled process by `harness_type` ("node" | "bun"), run work against it,
/// then `release` so the next caller can pick it up.
pub struct SharedRuntime {
    node_pool: RwLock<Vec<PooledProcess>>,
    bun_pool: RwLock<Vec<PooledProcess>>,
    max_per_type: usize,
    system: Arc<RwLock<sysinfo::System>>,
}

/// One slot in a `SharedRuntime` pool. `in_use` is flipped to true on
/// `acquire` and back to false on `release`; `last_used` is updated on
/// every state change.
#[derive(Debug, Clone)]
pub struct PooledProcess {
    pub pid: u32,
    pub name: String,
    pub in_use: bool,
    pub last_used: Instant,
}

impl SharedRuntime {
    pub fn new(max_per_type: usize) -> Self {
        Self {
            node_pool: RwLock::new(Vec::new()),
            bun_pool: RwLock::new(Vec::new()),
            max_per_type,
            system: Arc::new(RwLock::new(sysinfo::System::new_all())),
        }
    }

    pub async fn refresh(&self) {
        let mut sys = self.system.write().await;
        sys.refresh_all();
    }

    pub async fn acquire(&self, harness_type: &str) -> Result<PooledProcess> {
        let pool = match harness_type {
            "node" => &self.node_pool,
            "bun" => &self.bun_pool,
            _ => bail!("Unsupported harness type: {}. Use 'node' or 'bun'", harness_type),
        };

        let mut pool_guard = pool.write().await;

        if let Some(idx) = pool_guard.iter().position(|p| !p.in_use) {
            pool_guard[idx].in_use = true;
            pool_guard[idx].last_used = Instant::now();
            return Ok(pool_guard[idx].clone());
        }

        if pool_guard.len() < self.max_per_type {
            let (cmd, name) = match harness_type {
                "node" => ("node", "node"),
                "bun" => ("bun", "bun"),
                _ => unreachable!(),
            };

            let child = Command::new(cmd)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;

            let pid = child.id().unwrap_or(0);
            drop(child);

            tokio::time::sleep(Duration::from_millis(config::global().pool.spawn_delay_ms)).await;
            self.refresh().await;

            let sys = self.system.read().await;
            let pooled = if sys.process(Pid::from_u32(pid)).is_some() {
                PooledProcess {
                    pid,
                    name: name.to_string(),
                    in_use: true,
                    last_used: Instant::now(),
                }
            } else {
                bail!(
                    "Failed to spawn pooled process — check that '{}' is installed and on PATH",
                    harness_type
                );
            };

            pool_guard.push(pooled.clone());
            Ok(pooled)
        } else {
            bail!(
                "Pool exhausted: max {} instances of {} allowed. Set pool.max_per_type in config.toml to increase the limit.",
                self.max_per_type,
                harness_type
            )
        }
    }

    pub async fn release(&self, harness_type: &str, pid: u32) -> Result<()> {
        let pool = match harness_type {
            "node" => &self.node_pool,
            "bun" => &self.bun_pool,
            _ => bail!("Unsupported harness type: {}", harness_type),
        };

        let mut pool_guard = pool.write().await;
        if let Some(p) = pool_guard.iter_mut().find(|p| p.pid == pid) {
            p.in_use = false;
            p.last_used = Instant::now();
        }
        Ok(())
    }

    /// Pooled execution is unavailable until a command/worker protocol is wired.
    ///
    /// Do not acquire a worker here: the pool currently probes `--version`,
    /// which cannot execute a script or provide its output and exit status.
    pub async fn run_with_pool(
        &self,
        _harness_type: &str,
        _project: &str,
        _script: &str,
    ) -> Result<(u32, String)> {
        Err(Self::execution_unavailable().into())
    }

    /// Shared rejection for callers that must not even construct a runtime.
    pub(crate) fn execution_unavailable() -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "pooled run unavailable: no command execution backend is wired; no work was executed",
        )
    }

    pub async fn health_check(&self) -> RuntimeHealth {
        self.refresh().await;

        let sys = self.system.read().await;
        let node_pool = self.node_pool.read().await;
        let bun_pool = self.bun_pool.read().await;

        let mut healthy = true;
        let mut issues = Vec::new();

        for p in node_pool.iter().chain(bun_pool.iter()) {
            if let Some(proc) = sys.process(Pid::from_u32(p.pid)) {
                if proc.memory() > config::global().monitoring.per_process_warn_memory_bytes {
                    issues.push(format!(
                        "{} (PID {}) using {} MB - high memory",
                        p.name,
                        p.pid,
                        proc.memory() / 1024 / 1024
                    ));
                }
            } else {
                healthy = false;
                issues.push(format!("{} (PID {}) not found - may have crashed", p.name, p.pid));
            }
        }

        RuntimeHealth {
            healthy,
            issues,
            node_in_use: node_pool.iter().filter(|p| p.in_use).count(),
            bun_in_use: bun_pool.iter().filter(|p| p.in_use).count(),
        }
    }

    pub async fn status(&self) -> PoolStatus {
        let node_pool = self.node_pool.read().await;
        let bun_pool = self.bun_pool.read().await;

        PoolStatus {
            node_total: node_pool.len(),
            node_idle: node_pool.iter().filter(|p| !p.in_use).count(),
            bun_total: bun_pool.len(),
            bun_idle: bun_pool.iter().filter(|p| !p.in_use).count(),
            max_per_type: self.max_per_type,
        }
    }
}
