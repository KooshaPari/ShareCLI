//! Per-project resource limits (`ProjectLimits`), the manager that tracks
//! them across projects (`ProjectResources`), and the value type returned
//! by `check_limits` (`ResourceCheck`).
//!
//! `ProjectResources::check_limits` is the only consumer of sysinfo in
//! this module — it walks every process whose argv contains the project
//! name, sums memory + process count, and compares against the configured
//! caps. The `refresh` is intentionally fire-and-forget (`tokio::spawn`)
//! because the call is already on a hot path; the next reader of `system`
//! will pick up the fresh values once the spawn completes.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use crate::config;

/// Per-project memory and process-count caps, optionally pinned to a
/// subset of CPU cores (`cpu_affinity`).
#[derive(Debug, Clone)]
pub struct ProjectLimits {
    pub memory_limit_mb: u64,
    pub max_processes: usize,
    pub cpu_affinity: Option<Vec<usize>>,
}

impl Default for ProjectLimits {
    fn default() -> Self {
        let cfg = config::global();
        Self {
            memory_limit_mb: cfg.project_limits.memory_limit_mb,
            max_processes: cfg.project_limits.max_processes,
            cpu_affinity: None,
        }
    }
}

/// Project resource manager — per-project `ProjectLimits` plus the
/// `sysinfo::System` snapshot used to compute `check_limits` totals.
pub struct ProjectResources {
    projects: RwLock<HashMap<String, ProjectLimits>>,
    system: Arc<RwLock<sysinfo::System>>,
}

impl Default for ProjectResources {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectResources {
    pub fn new() -> Self {
        Self {
            projects: RwLock::new(HashMap::new()),
            system: Arc::new(RwLock::new(sysinfo::System::new_all())),
        }
    }

    pub async fn set_limits(&self, name: &str, limits: ProjectLimits) {
        let mut projects = self.projects.write().await;
        projects.insert(name.to_string(), limits);
    }

    pub async fn get_limits(&self, name: &str) -> ProjectLimits {
        let projects = self.projects.read().await;
        projects.get(name).cloned().unwrap_or_default()
    }

    pub async fn check_limits(&self, project: &str) -> Result<ResourceCheck> {
        self.refresh();
        let sys = self.system.read().await;
        let limits = self.get_limits(project).await;

        let mut total_memory = 0u64;
        let mut process_count = 0usize;

        for proc in sys.processes().values() {
            let cmd: Vec<String> =
                proc.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect();
            if cmd.iter().any(|c| c.contains(project)) {
                total_memory += proc.memory() / 1024 / 1024;
                process_count += 1;
            }
        }

        let memory_ok = total_memory <= limits.memory_limit_mb;
        let processes_ok = process_count <= limits.max_processes;

        Ok(ResourceCheck {
            memory_mb: total_memory,
            memory_limit_mb: limits.memory_limit_mb,
            memory_ok,
            process_count,
            max_processes: limits.max_processes,
            processes_ok,
            overall_ok: memory_ok && processes_ok,
        })
    }

    fn refresh(&self) {
        let sys = self.system.clone();
        tokio::spawn(async move {
            let mut s = sys.write().await;
            s.refresh_all();
        });
    }
}

/// Result of `ProjectResources::check_limits` — observed memory / process
/// count, configured limits, per-axis pass/fail, and an overall pass/fail
/// (true when both memory and process-count are within limits).
#[derive(Debug, Clone)]
pub struct ResourceCheck {
    pub memory_mb: u64,
    pub memory_limit_mb: u64,
    pub memory_ok: bool,
    pub process_count: usize,
    pub max_processes: usize,
    pub processes_ok: bool,
    pub overall_ok: bool,
}
