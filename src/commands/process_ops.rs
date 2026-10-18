//! `start` / `stop` / `run_pool` / `set_limits` / `check_limits` operator commands.
//!
//! These are the per-process CRUD-style operations exposed under
//! `sharecli start` / `sharecli stop` / `sharecli run-pool` /
//! `sharecli set-limits` / `sharecli check-limits`. They share a small
//! `force_kill_requires_confirmation` / `force_kill_preview` confirmation
//! pattern (C09 L81.6) and the `SHARED_RUNTIME` / `PROJECT_RESOURCES` singletons
//! (now in `runtime_helpers`).
//!
//! `expand_path` is the `~/` home-expansion helper used by `start` and the
//! `project` command (in `config_cmd`); colocated here because both
//! commands work with the same Config-driven path semantics.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use crate::commands::runtime_helpers::{get_project_resources, get_shared_runtime};
use crate::config::Config;
use crate::progress::StepProgress;
use crate::runtime::{ProcessFilter, ProcessPool, ProjectLimits};
use crate::spawn_policy::SpawnPolicy;

/// When `force` is set, destructive SIGKILL requires explicit `--yes` (C09 L81.6).
pub(crate) fn force_kill_requires_confirmation(force: bool, yes: bool) -> bool {
    force && !yes
}

/// Preview lines printed before aborting an unconfirmed force-kill.
pub(crate) fn force_kill_preview(target_label: &str, count: usize) {
    println!("Would force-kill (SIGKILL) {target_label} ({count} process(es)).");
    println!("Pass --yes to confirm. Prefer graceful stop without --force when possible.");
}

/// Start a harness process
pub async fn start(project: &str, harness: &str, cwd: Option<&str>, args: &[String]) -> Result<()> {
    let cfg = Config::load()?;

    let project_path = if let Some(c) = cwd {
        PathBuf::from(expand_path(c))
    } else if let Some(path) = cfg.projects.get(project) {
        PathBuf::from(expand_path(path))
    } else {
        anyhow::bail!(
            "Unknown project: {}. Add with 'sharecli project add <name> <path>'",
            project
        );
    };

    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {:?}", project_path);
    }

    // Apply the spawn-policy throttle when the harness is a build harness (cargo/rustc/…).
    // The policy is constructed from the global config's [spawn_policy] section.
    let pool = {
        let policy = SpawnPolicy::new(cfg.spawn_policy.clone());
        ProcessPool::with_spawn_policy(Arc::new(policy))
    };
    println!("Starting {} harness for project '{}'...", harness, project);

    let info = pool
        .spawn(
            harness,
            args,
            Some(project_path.clone()),
            Some(project.to_string()),
            Some(harness.to_string()),
        )
        .await?;

    println!("Started process {} ({})", info.pid, info.name);
    println!("Working directory: {:?}", project_path);

    Ok(())
}

/// Stop processes
pub async fn stop(
    pid: Option<u32>,
    project: Option<&str>,
    harness: Option<&str>,
    all: bool,
    force: bool,
    yes: bool,
) -> Result<()> {
    let pool = ProcessPool::new();

    if all {
        if force_kill_requires_confirmation(force, yes) {
            let count = pool.list().await.len();
            force_kill_preview("all managed processes", count);
            return Ok(());
        }
        println!("Stopping all managed processes{}...", if force { " (force)" } else { "" });
        pool.kill_all().await?;
        println!("All processes stopped.");
        return Ok(());
    }

    if let Some(p) = pid {
        if force_kill_requires_confirmation(force, yes) {
            force_kill_preview(&format!("PID {p}"), 1);
            return Ok(());
        }
        println!("Stopping process {p}{}...", if force { " (force)" } else { "" });
        pool.kill(p).await?;
        println!("Process {p} stopped.");
        return Ok(());
    }

    let filter = if let Some(proj) = project {
        ProcessFilter::ByProject(proj.to_string())
    } else if let Some(h) = harness {
        ProcessFilter::ByHarness(h.to_string())
    } else {
        anyhow::bail!("Specify --pid, --project, --harness, or --all to select what to stop");
    };

    let processes = pool.find(filter).await;
    if force_kill_requires_confirmation(force, yes) {
        let label = if let Some(proj) = project {
            format!("project '{proj}'")
        } else {
            format!("harness '{}'", harness.unwrap_or(""))
        };
        force_kill_preview(&label, processes.len());
        return Ok(());
    }

    let progress = StepProgress::new("Stopping processes", processes.len());
    let line_mode = progress.uses_line_output();
    for proc in &processes {
        pool.kill(proc.pid).await?;
        progress.inc(Some(&format!("{} ({})", proc.pid, proc.name)));
        if line_mode {
            println!("Stopped {} ({})", proc.pid, proc.name);
        }
    }
    progress.finish("Processes stopped");

    Ok(())
}

/// Run using pooled runtime
pub async fn run_pool(harness_type: &str, project: &str) -> Result<()> {
    let runtime = get_shared_runtime();
    let result = runtime.run_with_pool(harness_type, project, "").await?;
    println!("Pooled {} process {} for project {}", harness_type, result.0, project);
    println!("Output: {}", result.1);
    Ok(())
}

/// Set project limits
pub async fn set_limits(
    project: &str,
    memory_mb: Option<u64>,
    max_procs: Option<usize>,
) -> Result<()> {
    let resources = get_project_resources();
    let current = resources.get_limits(project).await;

    let memory_limit = memory_mb.unwrap_or(current.memory_limit_mb);
    let max_processes = max_procs.unwrap_or(current.max_processes);

    let limits = ProjectLimits {
        memory_limit_mb: memory_limit,
        max_processes,
        cpu_affinity: current.cpu_affinity,
    };

    resources.set_limits(project, limits).await;
    println!("Set limits for project '{}':", project);
    println!("  Memory: {} MB", memory_limit);
    println!("  Max processes: {}", max_processes);
    Ok(())
}

/// Check project limits
pub async fn check_limits(project: &str) -> Result<()> {
    let resources = get_project_resources();
    let check = resources.check_limits(project).await?;

    println!("=== Resource Limits for '{}' ===\n", project);

    println!("Memory: {} MB / {} MB", check.memory_mb, check.memory_limit_mb);
    if check.memory_ok {
        println!("  Status: OK");
    } else {
        println!("  Status: EXCEEDED (over by {} MB)", check.memory_mb - check.memory_limit_mb);
    }

    println!("\nProcesses: {} / {}", check.process_count, check.max_processes);
    if check.processes_ok {
        println!("  Status: OK");
    } else {
        println!("  Status: EXCEEDED (over by {})", check.process_count - check.max_processes);
    }

    println!("\nOverall: {}", if check.overall_ok { "OK" } else { "LIMIT EXCEEDED" });

    Ok(())
}

/// Expand a leading `~/` to the current user's home directory.
pub(crate) fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen("~/", &format!("{}/", home), 1);
        }
    }
    path.to_string()
}
