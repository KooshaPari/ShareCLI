// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 — Process & Pool Health Status (pool / health)
//! FR: FR-004
//!
//! Covers AC-004.2, AC-004.3.

use sharecli::runtime::{PoolStatus, RuntimeHealth, SharedRuntime};

/// Mirror of the pool table printed by `commands::pool_status`.
fn format_pool_status(status: &PoolStatus) -> String {
    let mut out = String::from("=== Shared Runtime Pool Status ===\n\n");
    out.push_str(&format!("{:<10} {:<10} {:<10} {:<10}\n", "TYPE", "TOTAL", "IDLE", "MAX"));
    out.push_str(&format!("{}\n", "-".repeat(40)));
    out.push_str(&format!(
        "{:<10} {:<10} {:<10} {:<10}\n",
        "node", status.node_total, status.node_idle, status.max_per_type
    ));
    out.push_str(&format!(
        "{:<10} {:<10} {:<10} {:<10}\n",
        "bun", status.bun_total, status.bun_idle, status.max_per_type
    ));
    out.push_str(&format!("\nMax per type: {}\n", status.max_per_type));
    out
}

fn format_health_label(health: &RuntimeHealth) -> String {
    if health.healthy {
        "Status: HEALTHY".to_string()
    } else {
        "Status: DEGRADED".to_string()
    }
}

fn format_health_probe(health: &RuntimeHealth, pool: &PoolStatus) -> String {
    let mut out = format!(
        "\nShared runtime health: {}\n",
        if health.healthy { "HEALTHY" } else { "DEGRADED" }
    );
    if health.issues.is_empty() {
        out.push_str("No runtime issues detected.\n");
    } else {
        out.push_str("\nIssues detected:\n");
        for issue in &health.issues {
            out.push_str(&format!("  - {issue}\n"));
        }
    }
    out.push_str("\nPool summary:\n");
    out.push_str(&format!(
        "  node: {} total, {} idle, {} in use\n",
        pool.node_total, pool.node_idle, health.node_in_use
    ));
    out.push_str(&format!(
        "  bun:  {} total, {} idle, {} in use\n",
        pool.bun_total, pool.bun_idle, health.bun_in_use
    ));
    out.push_str(&format!("\nMax per harness type: {}\n", pool.max_per_type));
    out
}

/// FR-004 / AC-004.2 — `pool` reports node/bun totals, idle counts, and max_per_type.
#[tokio::test]
async fn fr004_pool_reports_node_and_bun() {
    let max_per_type = 6usize;
    let runtime = SharedRuntime::new(max_per_type);
    let status = runtime.status().await;

    assert_eq!(status.node_total, 0);
    assert_eq!(status.node_idle, 0);
    assert_eq!(status.bun_total, 0);
    assert_eq!(status.bun_idle, 0);
    assert_eq!(status.max_per_type, max_per_type);

    let out = format_pool_status(&status);
    assert!(out.contains("=== Shared Runtime Pool Status ==="), "got: {out}");
    assert!(
        out.contains("TYPE")
            && out.contains("TOTAL")
            && out.contains("IDLE")
            && out.contains("MAX"),
        "pool table columns MUST be present; got: {out}"
    );
    assert!(out.contains("node") && out.contains("bun"), "got: {out}");
    assert!(
        out.contains(&format!("Max per type: {max_per_type}")),
        "pool MUST report max_per_type ceiling; got: {out}"
    );
}

/// FR-004 / AC-004.3 — health reports HEALTHY/DEGRADED from `RuntimeHealth::healthy`.
#[tokio::test]
async fn fr004_health_reports_healthy_or_degraded() {
    let runtime = SharedRuntime::new(3);
    let pool = runtime.status().await;
    let live = runtime.health_check().await;

    assert!(live.healthy, "empty shared pool MUST be healthy (no dead/high-mem members)");
    assert!(live.issues.is_empty());
    assert_eq!(live.node_in_use, 0);
    assert_eq!(live.bun_in_use, 0);

    let healthy_out = format_health_probe(&live, &pool);
    assert!(
        healthy_out.contains("HEALTHY"),
        "empty pool health MUST print HEALTHY; got: {healthy_out}"
    );
    assert!(format_health_label(&live).contains("HEALTHY"));

    let degraded = RuntimeHealth {
        healthy: false,
        issues: vec!["node (PID 999999) not found - may have crashed".to_string()],
        node_in_use: 1,
        bun_in_use: 0,
    };
    let degraded_out = format_health_probe(&degraded, &pool);
    assert!(
        degraded_out.contains("DEGRADED"),
        "unhealthy RuntimeHealth MUST print DEGRADED; got: {degraded_out}"
    );
    assert!(degraded_out.contains("not found"), "issues MUST surface; got: {degraded_out}");
    assert!(format_health_label(&degraded).contains("DEGRADED"));
    assert!(!format_health_label(&degraded).contains("HEALTHY"));
}
