// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 × FR-011 — agent RSS samples tighten thermal spawn gate
//! FR: FR-006, FR-011
//!
//! AC-006.12 aggregate watched-agent RSS escalates Hypervisor gate tier

use std::sync::Arc;

use sharecli_core::{
    AgentAwareThermalGate, FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest,
    ThermalDecision, THERMAL_MAX_RETRIES,
};
use sharecli_fleet::agent_contention::{
    agent_resource_contention_tier, combined_agent_contention_tier, AgentContentionThresholds,
    AgentContentionTier, AgentResourceThresholds,
};
use tempfile::TempDir;

/// FR-006 / AC-006.12 — RSS refuse tier maps to Hypervisor Refuse.
#[tokio::test(start_paused = true)]
async fn fr006_agent_rss_refuses_hypervisor_spawn() {
    fn rss_refuse_tier() -> AgentContentionTier {
        agent_resource_contention_tier(
            3_000,
            AgentResourceThresholds { warn_total_rss_bytes: 1_000, refuse_total_rss_bytes: 2_000 },
        )
    }
    let dir = TempDir::new().expect("tempdir");
    let inner = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let gate = Arc::new(AgentAwareThermalGate::with_agent_tier(inner, Arc::new(rss_refuse_tier)));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "rss-gated".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "rss-gated".to_string()];

    let err = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect_err("RSS refuse MUST err after retries");

    let msg = err.to_string();
    assert!(
        msg.contains("thermally throttled"),
        "error must mention thermally throttled, got {msg}; max_retries={THERMAL_MAX_RETRIES}"
    );
}

/// FR-006 / AC-006.12 — combined tier uses max(count, RSS).
#[test]
fn fr006_combined_tier_rss_refuse_with_low_count() {
    let tier = combined_agent_contention_tier(
        1,
        40_000_000_000,
        AgentContentionThresholds::default(),
        AgentResourceThresholds::default(),
    );
    assert_eq!(tier, AgentContentionTier::Refuse);
}
