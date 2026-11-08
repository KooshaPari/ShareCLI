// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Thermal gate — pre-spawn back-pressure seam for the hypervisor.
//!
//! The hypervisor queries a [`ThermalGate`] before every spawn to decide whether
//! the device has enough thermal headroom.  The production implementation
//! ([`SystemThermalGate`]) wraps the platform thermal governor from
//! `sharecli-fleet`; the agent-contention wrapper ([`AgentAwareThermalGate`])
//! additionally escalates the decision when many known agents are contending
//! for resources (FR-011).
//!
//! Tests inject [`FakeThermalGate`] for deterministic Green / Yellow / Red
//! coverage.

use std::sync::Arc;

use sharecli_fleet::agent_contention::{
    agent_contention_tier, live_agent_contention_tier, AgentContentionThresholds,
    AgentContentionTier,
};
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use tracing::warn;

// ---------------------------------------------------------------------------
// Decision enum + trait
// ---------------------------------------------------------------------------

/// Decision returned by a [`ThermalGate`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalDecision {
    /// Device is cool — spawn may proceed unconditionally.
    Allow,
    /// Device is warm — spawn may proceed but caller should log a warning.
    Warn,
    /// Device is hot — spawn must be back-pressured or refused.
    Refuse,
}

/// A seam that the [`Hypervisor`](crate::Hypervisor) calls before every spawn
/// to determine whether the device has enough thermal headroom.
///
/// The production implementation ([`SystemThermalGate`]) delegates to
/// [`ThermalGovernor`] from `sharecli-fleet`.  Tests inject a fake via
/// [`FakeThermalGate`].
pub trait ThermalGate: Send + Sync {
    /// Poll the current thermal state and return a spawn decision.
    fn check(&self) -> ThermalDecision;
}

// ---------------------------------------------------------------------------
// Production gate (SystemThermalGate)
// ---------------------------------------------------------------------------

/// Production [`ThermalGate`] — wraps [`ThermalGovernor`] from `sharecli-fleet`.
///
/// Maps `ThermalLevel::Green` → [`ThermalDecision::Allow`],
///      `ThermalLevel::Yellow` → [`ThermalDecision::Warn`],
///      `ThermalLevel::Red`    → [`ThermalDecision::Refuse`].
///
/// If `poll()` returns an error (e.g. missing sysctl) the gate defaults to
/// [`ThermalDecision::Allow`] and logs a warning so that sysctl unavailability
/// does not block spawns on non-macOS CI runners.
#[derive(Debug)]
pub struct SystemThermalGate {
    governor: ThermalGovernor,
}

impl Default for SystemThermalGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemThermalGate {
    /// Create a new gate backed by the real [`ThermalGovernor`].
    pub fn new() -> Self {
        Self { governor: ThermalGovernor::new() }
    }
}

impl ThermalGate for SystemThermalGate {
    fn check(&self) -> ThermalDecision {
        match self.governor.poll() {
            Ok(ThermalLevel::Green) => ThermalDecision::Allow,
            Ok(ThermalLevel::Yellow) => ThermalDecision::Warn,
            Ok(ThermalLevel::Red) => ThermalDecision::Refuse,
            Err(e) => {
                warn!(err = %e, "thermal-gate: poll failed — defaulting to Allow");
                ThermalDecision::Allow
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Agent-contention escalation
// ---------------------------------------------------------------------------

/// Combine hardware thermal gate output with host agent-count contention (FR-011).
pub fn combine_thermal_agent_decision(
    thermal: ThermalDecision,
    agents: AgentContentionTier,
) -> ThermalDecision {
    use AgentContentionTier::{Ok as AgentsOk, Refuse as AgentsRefuse, Warn as AgentsWarn};
    use ThermalDecision::{Allow, Refuse, Warn};
    match (thermal, agents) {
        (Refuse, _) | (_, AgentsRefuse) => Refuse,
        (Allow, AgentsWarn) => Warn,
        (Allow, AgentsOk) => Allow,
        (Warn, _) => Warn,
    }
}

/// Production gate: thermal governor + live host agent inventory escalation.
///
/// Wraps an inner [`ThermalGate`] (typically [`SystemThermalGate`]) and escalates
/// Allow→Warn or any→Refuse when proc-scan agent count or aggregate RSS crosses
/// configured thresholds.
pub struct AgentAwareThermalGate {
    inner: Arc<dyn ThermalGate>,
    agent_tier: Arc<dyn Fn() -> AgentContentionTier + Send + Sync>,
}

impl AgentAwareThermalGate {
    /// Gate with live proc scan + resource samples on every check.
    pub fn new(inner: Arc<dyn ThermalGate>, _thresholds: AgentContentionThresholds) -> Self {
        Self { inner, agent_tier: Arc::new(live_agent_contention_tier) }
    }

    /// Test/injection hook — supply a deterministic agent-count-only tier fn.
    pub fn with_agent_count(
        inner: Arc<dyn ThermalGate>,
        thresholds: AgentContentionThresholds,
        agent_count: fn() -> usize,
    ) -> Self {
        Self {
            inner,
            agent_tier: Arc::new(move || agent_contention_tier(agent_count(), thresholds)),
        }
    }

    /// Test/injection hook — supply an explicit contention tier source.
    pub fn with_agent_tier(
        inner: Arc<dyn ThermalGate>,
        agent_tier: Arc<dyn Fn() -> AgentContentionTier + Send + Sync>,
    ) -> Self {
        Self { inner, agent_tier }
    }
}

impl ThermalGate for AgentAwareThermalGate {
    fn check(&self) -> ThermalDecision {
        let base = self.inner.check();
        let tier = (self.agent_tier)();
        let effective = combine_thermal_agent_decision(base, tier);
        if tier != AgentContentionTier::Ok && effective != base {
            tracing::warn!(
                ?tier,
                ?base,
                ?effective,
                "thermal-gate: host agent contention escalated spawn decision"
            );
        }
        effective
    }
}

// ---------------------------------------------------------------------------
// Retry constants (used by Hypervisor::thermal_gate_check)
// ---------------------------------------------------------------------------

/// How many times the hypervisor will sleep-retry when the gate returns
/// [`ThermalDecision::Refuse`] before giving up with an explicit error.
pub const THERMAL_MAX_RETRIES: u32 = 5;

/// Duration of each sleep in the thermal back-pressure retry loop.
///
/// 2 s per attempt → up to ~10 s total wait before a hard error is returned.
pub const THERMAL_RETRY_SLEEP: std::time::Duration = std::time::Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Test-only FakeThermalGate
// ---------------------------------------------------------------------------

/// A controllable [`ThermalGate`] for unit tests.
///
/// The gate's decision is set at construction time and never changes, making it
/// suitable for table-driven tests that need deterministic thermal states.
#[derive(Debug)]
pub struct FakeThermalGate {
    decision: ThermalDecision,
}

impl FakeThermalGate {
    /// Create a fake gate that always returns `decision`.
    pub fn new(decision: ThermalDecision) -> Self {
        Self { decision }
    }
}

impl ThermalGate for FakeThermalGate {
    fn check(&self) -> ThermalDecision {
        self.decision
    }
}

// ---------------------------------------------------------------------------
// Pure-thermal unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// FakeThermalGate::check() must return the decision it was constructed with.
    #[test]
    fn fake_thermal_gate_returns_configured_decision() {
        for decision in [ThermalDecision::Allow, ThermalDecision::Warn, ThermalDecision::Refuse] {
            let gate = FakeThermalGate::new(decision);
            assert_eq!(gate.check(), decision);
        }
    }

    /// ThermalDecision variants are Copy + Eq — spot-check the impls.
    #[test]
    fn thermal_decision_copy_eq() {
        let a = ThermalDecision::Allow;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(ThermalDecision::Allow, ThermalDecision::Refuse);
    }

    #[test]
    fn combine_thermal_agent_decision_escalates_allow_to_warn() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Warn),
            ThermalDecision::Warn
        );
    }

    #[test]
    fn combine_thermal_agent_decision_agent_refuse_overrides_allow() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Allow, AgentContentionTier::Refuse),
            ThermalDecision::Refuse
        );
    }

    /// combine_thermal_agent_decision: Warn thermal + AgentsOk stays Warn.
    #[test]
    fn combine_thermal_agent_decision_warn_agents_ok_stays_warn() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Warn, AgentContentionTier::Ok),
            ThermalDecision::Warn
        );
    }

    /// combine_thermal_agent_decision: Warn thermal + AgentsRefuse -> Refuse.
    #[test]
    fn combine_thermal_agent_decision_warn_agents_refuse_escalates() {
        assert_eq!(
            combine_thermal_agent_decision(ThermalDecision::Warn, AgentContentionTier::Refuse),
            ThermalDecision::Refuse
        );
    }
}
