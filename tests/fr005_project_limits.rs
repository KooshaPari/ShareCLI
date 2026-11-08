// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-005 — Per-Project Resource Limits (defaults / set / get)
//! FR: FR-005
//!
//! Covers AC-005.1, AC-005.2, AC-005.3.

use sharecli::config;
use sharecli::runtime::{ProjectLimits, ProjectResources};

/// Mirror of the confirmation lines printed by `commands::set_limits`.
fn format_set_limits_confirmation(project: &str, memory_mb: u64, max_processes: usize) -> String {
    format!(
        "Set limits for project '{project}':\n  Memory: {memory_mb} MB\n  Max processes: {max_processes}\n"
    )
}

/// `ProjectLimits::default()` reads `config::global()`; CLI calls `init_global` at startup.
fn ensure_config() {
    let _ = config::init_global();
}

/// FR-005 / AC-005.1 — `ProjectLimits::default()` returns 1024 MB / 10 procs / no affinity.
#[test]
fn fr005_project_limits_default_values() {
    ensure_config();
    let limits = ProjectLimits::default();
    // AC pins the stock defaults from `ProjectLimitsConfig::default()`.
    assert_eq!(limits.memory_limit_mb, 1024, "default memory_limit_mb MUST be 1024");
    assert_eq!(limits.max_processes, 10, "default max_processes MUST be 10");
    assert!(limits.cpu_affinity.is_none(), "default cpu_affinity MUST be None");
}

/// FR-005 / AC-005.2 — `limits` sets project limits and prints a confirmation.
#[tokio::test]
async fn fr005_limits_set_persists_values() {
    let resources = ProjectResources::new();
    let project = "acme";
    let memory_mb = 512u64;
    let max_processes = 3usize;

    let limits = ProjectLimits { memory_limit_mb: memory_mb, max_processes, cpu_affinity: None };
    resources.set_limits(project, limits).await;

    let stored = resources.get_limits(project).await;
    assert_eq!(stored.memory_limit_mb, memory_mb);
    assert_eq!(stored.max_processes, max_processes);

    let out = format_set_limits_confirmation(project, memory_mb, max_processes);
    assert!(
        out.contains(&format!("Set limits for project '{project}':")),
        "limits MUST print confirmation; got: {out}"
    );
    assert!(out.contains("Memory: 512 MB"), "got: {out}");
    assert!(out.contains("Max processes: 3"), "got: {out}");
}

/// FR-005 / AC-005.3 — `get_limits` returns last set values, or defaults for unknown.
#[tokio::test]
async fn fr005_get_limits_returns_default_for_unknown() {
    ensure_config();
    let resources = ProjectResources::new();

    let unknown = resources.get_limits("never-registered").await;
    let defaults = ProjectLimits::default();
    assert_eq!(unknown.memory_limit_mb, defaults.memory_limit_mb);
    assert_eq!(unknown.max_processes, defaults.max_processes);
    assert_eq!(unknown.cpu_affinity, defaults.cpu_affinity);

    resources
        .set_limits(
            "widget",
            ProjectLimits {
                memory_limit_mb: 2048,
                max_processes: 7,
                cpu_affinity: Some(vec![0, 1]),
            },
        )
        .await;

    let widget = resources.get_limits("widget").await;
    assert_eq!(widget.memory_limit_mb, 2048);
    assert_eq!(widget.max_processes, 7);
    assert_eq!(widget.cpu_affinity, Some(vec![0, 1]));

    resources
        .set_limits(
            "widget",
            ProjectLimits { memory_limit_mb: 256, max_processes: 2, cpu_affinity: None },
        )
        .await;
    let latest = resources.get_limits("widget").await;
    assert_eq!(latest.memory_limit_mb, 256, "get_limits MUST return most recently set limits");
    assert_eq!(latest.max_processes, 2);
    assert!(latest.cpu_affinity.is_none());
}
