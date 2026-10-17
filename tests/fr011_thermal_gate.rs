// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-011 — Thermal Contention Gate
//! FR: FR-011
//!
//! AC-011.1 ThermalGovernor mock poll
//! AC-011.2 FakeThermalGate stable decisions
//! AC-011.3 Hypervisor Refuse → thermally throttled (see also AC-008.3)

use std::sync::Arc;

use sharecli_core::{
    FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision, ThermalGate,
    THERMAL_MAX_RETRIES,
};
use sharecli_fleet::{ThermalGovernor, ThermalLevel};
use tempfile::TempDir;

/// FR-011 / AC-011.1 — mock governor returns configured level.
#[test]
fn fr011_thermal_governor_mock_poll() {
    for level in [ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red] {
        let gov = ThermalGovernor::with_mock(level);
        let got = gov.poll().expect("mock poll");
        assert_eq!(got, level);
    }
}

/// FR-011 / AC-011.2 — FakeThermalGate decisions are stable.
#[test]
fn fr011_fake_thermal_gate_stable() {
    let refuse = FakeThermalGate::new(ThermalDecision::Refuse);
    assert_eq!(refuse.check(), ThermalDecision::Refuse);
    assert_eq!(refuse.check(), ThermalDecision::Refuse);

    let allow = FakeThermalGate::new(ThermalDecision::Allow);
    assert_eq!(allow.check(), ThermalDecision::Allow);
}

/// FR-011 / AC-011.3 — Refuse fails loudly before speculative work.
#[tokio::test(start_paused = true)]
async fn fr011_refuse_thermally_throttled() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Refuse));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "gated".to_string()];
    #[cfg(windows)]
    let argv = vec!["cmd".to_string(), "/C".to_string(), "echo".to_string(), "gated".to_string()];

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
