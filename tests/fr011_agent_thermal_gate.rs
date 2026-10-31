// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-011 — thermal gate + host agent contention (FR-006 × FR-011)
//! FR: FR-011
//!
//! AC-011.4 Hypervisor escalates spawn gate when proc-scan agent count exceeds thresholds.

use std::sync::Arc;

use sharecli_core::{
    AgentAwareThermalGate, FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest,
    ThermalDecision, THERMAL_MAX_RETRIES,
};
use sharecli_fleet::agent_contention::{effective_gate_decision, AgentContentionThresholds};
use sharecli_fleet::thermal::ThermalLevel;
use tempfile::TempDir;

/// FR-011 / AC-011.4 — agent refuse tier maps to Hypervisor Refuse (integration).
#[tokio::test(start_paused = true)]
async fn fr011_agent_contention_refuses_hypervisor_spawn() {
    fn at_refuse_limit() -> usize {
        AgentContentionThresholds::default().refuse_at
    }
    let dir = TempDir::new().expect("tempdir");
    let inner = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let gate = Arc::new(AgentAwareThermalGate::with_agent_count(
        inner,
        AgentContentionThresholds::default(),
        at_refuse_limit,
    ));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "agent-gated".to_string()];
    #[cfg(windows)]
    let argv =
        vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "agent-gated".to_string()];

    let err = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect_err("Refuse MUST err after retries");

    let msg = err.to_string();
    assert!(
        msg.contains("thermally throttled"),
        "error must mention thermally throttled, got {msg}; max_retries={THERMAL_MAX_RETRIES}"
    );
}

/// FR-011 / AC-011.4 — agent warn tier escalates Allow→Warn but still spawns.
#[tokio::test]
async fn fr011_agent_contention_warn_still_spawns() {
    fn at_warn_limit() -> usize {
        AgentContentionThresholds::default().warn_at
    }
    let dir = TempDir::new().expect("tempdir");
    let inner = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let gate = Arc::new(AgentAwareThermalGate::with_agent_count(
        inner,
        AgentContentionThresholds::default(),
        at_warn_limit,
    ));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "warn-ok".to_string()];
    #[cfg(windows)]
    let argv = vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "warn-ok".to_string()];

    hv.run(SpawnRequest {
        argv,
        cwd: dir.path().to_path_buf(),
        env: vec![],
        queue_priority: QueuePriority::Normal,
    })
    .await
    .expect("Warn tier MUST still allow spawn");
}

/// FR-011 / AC-011.4 — thermal TUI effective gate denies on agent refuse + Green thermal.
#[test]
fn fr011_effective_gate_decision_agent_refuse() {
    assert_eq!(
        effective_gate_decision(
            ThermalLevel::Green,
            AgentContentionThresholds::default().refuse_at
        ),
        "DENY"
    );
}
