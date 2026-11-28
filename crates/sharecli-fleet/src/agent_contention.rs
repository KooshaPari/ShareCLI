// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Host agent-count contention for thermal+agent gating (FR-011).
//!
//! When many coding agents run concurrently, speculative coalesce pressure rises
//! even if raw thermal readings are still Green. This module maps proc-scan agent
//! inventory size and live RSS samples to escalation tiers consumed by
//! [`sharecli_core::AgentAwareThermalGate`].

use crate::proc_scan::scan_host_agents;
use crate::resource_watch::watch_host_agents;
use crate::thermal::ThermalLevel;

/// Thresholds for host agent inventory escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentContentionThresholds {
    /// At or above: escalate Allow → Warn (Hypervisor still proceeds).
    pub warn_at: usize,
    /// At or above: escalate to Refuse (Hypervisor back-pressures / errors).
    pub refuse_at: usize,
}

impl Default for AgentContentionThresholds {
    /// Baseline calibrated for one developer host running a whole agent fleet.
    ///
    /// The former 4/8 baseline denied at 8 agents, a count a normal host
    /// reaches routinely (17 concurrent agents measured on a 16-core host), so
    /// the gate refused normal work. WARN at 16 stays informative just below
    /// the documented 20-agent device cap; REFUSE at 32 only trips at twice
    /// that cap.
    fn default() -> Self {
        Self { warn_at: 16, refuse_at: 32 }
    }
}

impl AgentContentionThresholds {
    /// Resolve thresholds from the environment, falling back to [`Self::default`].
    ///
    /// Overrides: `SHARECLI_AGENT_CONTENTION_WARN_AT`,
    /// `SHARECLI_AGENT_CONTENTION_REFUSE_AT`.
    pub fn from_env() -> Self {
        Self::from_overrides(
            env_parse("SHARECLI_AGENT_CONTENTION_WARN_AT"),
            env_parse("SHARECLI_AGENT_CONTENTION_REFUSE_AT"),
        )
    }

    /// Pure resolver behind [`Self::from_env`]; testable without mutating
    /// process-global environment state.
    pub fn from_overrides(warn_at: Option<usize>, refuse_at: Option<usize>) -> Self {
        let defaults = Self::default();
        let warn_at = warn_at.unwrap_or(defaults.warn_at);
        let refuse_at = refuse_at.unwrap_or(defaults.refuse_at);
        if refuse_at == 0 || warn_at >= refuse_at {
            tracing::warn!(
                warn_at,
                refuse_at,
                "invalid agent contention thresholds (need 0 < warn_at < refuse_at); using defaults"
            );
            return defaults;
        }
        Self { warn_at, refuse_at }
    }
}

/// Thresholds for aggregate detected-agent RSS escalation (AC-006.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentResourceThresholds {
    /// Sum of watched agent RSS at or above: escalate Allow → Warn.
    pub warn_total_rss_bytes: u64,
    /// Sum of watched agent RSS at or above: escalate to Refuse.
    pub refuse_total_rss_bytes: u64,
}

impl Default for AgentResourceThresholds {
    fn default() -> Self {
        Self {
            warn_total_rss_bytes: 16 * 1_073_741_824,
            refuse_total_rss_bytes: 32 * 1_073_741_824,
        }
    }
}

impl AgentResourceThresholds {
    /// Resolve RSS thresholds from the environment, falling back to defaults.
    ///
    /// Overrides: `SHARECLI_AGENT_RSS_WARN_BYTES`,
    /// `SHARECLI_AGENT_RSS_REFUSE_BYTES`.
    pub fn from_env() -> Self {
        Self::from_overrides(
            env_parse("SHARECLI_AGENT_RSS_WARN_BYTES"),
            env_parse("SHARECLI_AGENT_RSS_REFUSE_BYTES"),
        )
    }

    /// Pure resolver behind [`Self::from_env`].
    pub fn from_overrides(
        warn_total_rss_bytes: Option<u64>,
        refuse_total_rss_bytes: Option<u64>,
    ) -> Self {
        let defaults = Self::default();
        let warn = warn_total_rss_bytes.unwrap_or(defaults.warn_total_rss_bytes);
        let refuse = refuse_total_rss_bytes.unwrap_or(defaults.refuse_total_rss_bytes);
        if refuse == 0 || warn >= refuse {
            tracing::warn!(
                warn,
                refuse,
                "invalid agent RSS thresholds (need 0 < warn < refuse); using defaults"
            );
            return defaults;
        }
        Self { warn_total_rss_bytes: warn, refuse_total_rss_bytes: refuse }
    }
}

/// Parse a threshold override from the environment, ignoring garbage values.
fn env_parse<T: std::str::FromStr>(name: &str) -> Option<T> {
    let raw = std::env::var(name).ok()?;
    match raw.trim().parse::<T>() {
        Ok(value) => Some(value),
        Err(_) => {
            tracing::warn!(name, value = %raw, "ignoring unparseable threshold override");
            None
        }
    }
}

/// Contention tier derived from live agent count and/or RSS pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentContentionTier {
    Ok,
    Warn,
    Refuse,
}

/// Map host agent count to a contention tier.
pub fn agent_contention_tier(
    count: usize,
    thresholds: AgentContentionThresholds,
) -> AgentContentionTier {
    if count >= thresholds.refuse_at {
        AgentContentionTier::Refuse
    } else if count >= thresholds.warn_at {
        AgentContentionTier::Warn
    } else {
        AgentContentionTier::Ok
    }
}

/// Map aggregate watched-agent RSS to a contention tier.
pub fn agent_resource_contention_tier(
    total_rss_bytes: u64,
    thresholds: AgentResourceThresholds,
) -> AgentContentionTier {
    if total_rss_bytes >= thresholds.refuse_total_rss_bytes {
        AgentContentionTier::Refuse
    } else if total_rss_bytes >= thresholds.warn_total_rss_bytes {
        AgentContentionTier::Warn
    } else {
        AgentContentionTier::Ok
    }
}

/// Combine count- and RSS-derived tiers; the higher tier wins.
pub fn combined_agent_contention_tier(
    count: usize,
    total_rss_bytes: u64,
    count_thresholds: AgentContentionThresholds,
    rss_thresholds: AgentResourceThresholds,
) -> AgentContentionTier {
    let count_tier = agent_contention_tier(count, count_thresholds);
    let rss_tier = agent_resource_contention_tier(total_rss_bytes, rss_thresholds);
    count_tier.max(rss_tier)
}

/// Live host agent inventory size (FR-006 proc scan).
pub fn count_host_agents() -> usize {
    scan_host_agents().len()
}

/// Sum RSS across live watched agent PIDs (FR-006 × FR-011).
pub fn total_watched_agent_rss_bytes() -> u64 {
    watch_host_agents().iter().map(|row| row.resource.mem_rss_bytes).sum()
}

/// Live combined contention tier from proc scan + resource samples.
pub fn live_agent_contention_tier() -> AgentContentionTier {
    combined_agent_contention_tier(
        count_host_agents(),
        total_watched_agent_rss_bytes(),
        AgentContentionThresholds::from_env(),
        AgentResourceThresholds::from_env(),
    )
}

/// Operator-facing ADMIT/DENY label matching Hypervisor gate semantics.
///
/// Red thermal or agent Refuse tier → DENY; otherwise ADMIT (including Yellow
/// thermal and agent Warn tier — those still proceed with warnings).
pub fn effective_gate_decision_for_tier(
    thermal: ThermalLevel,
    tier: AgentContentionTier,
) -> &'static str {
    if thermal == ThermalLevel::Red {
        return "DENY";
    }
    if tier == AgentContentionTier::Refuse {
        return "DENY";
    }
    "ADMIT"
}

/// Operator-facing ADMIT/DENY from thermal + agent count (legacy callers).
pub fn effective_gate_decision(thermal: ThermalLevel, agent_count: usize) -> &'static str {
    let tier = agent_contention_tier(agent_count, AgentContentionThresholds::default());
    effective_gate_decision_for_tier(thermal, tier)
}

fn thermal_level_label(level: ThermalLevel) -> &'static str {
    match level {
        ThermalLevel::Green => "GREEN",
        ThermalLevel::Yellow => "YELLOW",
        ThermalLevel::Red => "RED",
    }
}

fn contention_tier_label(tier: AgentContentionTier) -> &'static str {
    match tier {
        AgentContentionTier::Ok => "OK",
        AgentContentionTier::Warn => "WARN",
        AgentContentionTier::Refuse => "REFUSE",
    }
}

/// Structured gate fields for JSON report / programmatic surfaces (FR-011).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GateStatusSnapshot {
    /// Thermal level label (`GREEN` / `YELLOW` / `RED`).
    pub thermal_pressure: String,
    /// Live proc-scan agent inventory size.
    pub detected_agents: usize,
    /// Sum of watched agent RSS bytes.
    pub agent_total_rss_bytes: u64,
    /// Agent contention tier label (`OK` / `WARN` / `REFUSE`).
    pub agent_contention: String,
    /// Effective ADMIT/DENY decision.
    pub gate_decision: String,
}

/// Build structured gate fields from live thermal level + agent inventory.
pub fn gate_status_snapshot(thermal: ThermalLevel, agent_count: usize) -> GateStatusSnapshot {
    gate_status_snapshot_with_rss(thermal, agent_count, total_watched_agent_rss_bytes())
}

/// Build gate snapshot with explicit RSS (tests / injection).
pub fn gate_status_snapshot_with_rss(
    thermal: ThermalLevel,
    agent_count: usize,
    total_rss_bytes: u64,
) -> GateStatusSnapshot {
    let tier = combined_agent_contention_tier(
        agent_count,
        total_rss_bytes,
        AgentContentionThresholds::from_env(),
        AgentResourceThresholds::from_env(),
    );
    GateStatusSnapshot {
        thermal_pressure: thermal_level_label(thermal).to_string(),
        detected_agents: agent_count,
        agent_total_rss_bytes: total_rss_bytes,
        agent_contention: contention_tier_label(tier).to_string(),
        gate_decision: effective_gate_decision_for_tier(thermal, tier).to_string(),
    }
}

impl GateStatusSnapshot {
    /// Companion CSV block appended after agent inventory rows (FR-007 / AC-007.19).
    pub fn format_csv_companion(&self) -> String {
        format!(
            "\nrecord,thermal_pressure,detected_agents,agent_total_rss_bytes,agent_contention,gate_decision\n\
             gate,{},{},{},{},{}\n",
            self.thermal_pressure,
            self.detected_agents,
            self.agent_total_rss_bytes,
            self.agent_contention,
            self.gate_decision,
        )
    }
}

/// Operator status section from a captured gate snapshot (FR-007 / AC-007.17).
pub fn format_gate_status_from_snapshot(snap: &GateStatusSnapshot) -> String {
    let count_thresholds = AgentContentionThresholds::from_env();
    let rss_thresholds = AgentResourceThresholds::from_env();
    format!(
        "\n=== Thermal Gate (FR-011) ===\n\n\
         Thermal level: {}\n\
         Detected agents: {}\n\
         Agent RSS total: {} bytes\n\
         Agent contention: {} (count warn>={}, refuse>={}; RSS warn>={}, refuse>={})\n\
         Gate decision: [{}]\n",
        snap.thermal_pressure,
        snap.detected_agents,
        snap.agent_total_rss_bytes,
        snap.agent_contention,
        count_thresholds.warn_at,
        count_thresholds.refuse_at,
        rss_thresholds.warn_total_rss_bytes,
        rss_thresholds.refuse_total_rss_bytes,
        snap.gate_decision,
    )
}

/// Operator status section: thermal level, agent inventory, effective gate (FR-011).
pub fn format_gate_status_section(thermal: ThermalLevel, agent_count: usize) -> String {
    format_gate_status_from_snapshot(&gate_status_snapshot(thermal, agent_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_contention_tier_boundaries() {
        let t = AgentContentionThresholds { warn_at: 4, refuse_at: 8 };
        assert_eq!(agent_contention_tier(0, t), AgentContentionTier::Ok);
        assert_eq!(agent_contention_tier(3, t), AgentContentionTier::Ok);
        assert_eq!(agent_contention_tier(4, t), AgentContentionTier::Warn);
        assert_eq!(agent_contention_tier(7, t), AgentContentionTier::Warn);
        assert_eq!(agent_contention_tier(8, t), AgentContentionTier::Refuse);
    }

    #[test]
    fn agent_resource_contention_tier_boundaries() {
        let t =
            AgentResourceThresholds { warn_total_rss_bytes: 1_000, refuse_total_rss_bytes: 2_000 };
        assert_eq!(agent_resource_contention_tier(500, t), AgentContentionTier::Ok);
        assert_eq!(agent_resource_contention_tier(1_000, t), AgentContentionTier::Warn);
        assert_eq!(agent_resource_contention_tier(2_000, t), AgentContentionTier::Refuse);
    }

    #[test]
    fn combined_tier_takes_max_of_count_and_rss() {
        let count_t = AgentContentionThresholds { warn_at: 4, refuse_at: 8 };
        let rss_t =
            AgentResourceThresholds { warn_total_rss_bytes: 1_000, refuse_total_rss_bytes: 2_000 };
        assert_eq!(
            combined_agent_contention_tier(0, 2_500, count_t, rss_t),
            AgentContentionTier::Refuse,
            "RSS refuse MUST escalate even with low agent count"
        );
        assert_eq!(
            combined_agent_contention_tier(8, 0, count_t, rss_t),
            AgentContentionTier::Refuse,
            "count refuse MUST escalate even with low RSS"
        );
    }

    #[test]
    fn effective_gate_decision_agent_refuse_denies_on_green() {
        let refuse_at = AgentContentionThresholds::default().refuse_at;
        assert_eq!(
            effective_gate_decision(ThermalLevel::Green, refuse_at),
            "DENY",
            "agent refuse tier MUST deny even when thermal is Green"
        );
    }

    #[test]
    fn effective_gate_decision_agent_warn_still_admits() {
        let warn_at = AgentContentionThresholds::default().warn_at;
        assert_eq!(
            effective_gate_decision(ThermalLevel::Green, warn_at),
            "ADMIT",
            "agent warn tier MUST still ADMIT (Warn proceeds in Hypervisor)"
        );
    }

    #[test]
    fn format_gate_status_section_includes_decision() {
        let refuse_at = AgentContentionThresholds::default().refuse_at;
        let section = format_gate_status_section(ThermalLevel::Green, refuse_at);
        assert!(section.contains("=== Thermal Gate (FR-011) ==="));
        assert!(section.contains("Thermal level: GREEN"));
        assert!(section.contains(&format!("Detected agents: {refuse_at}")));
        assert!(section.contains("Agent contention: REFUSE"));
        assert!(section.contains("Gate decision: [DENY]"));
    }

    #[test]
    fn gate_status_snapshot_format_csv_companion() {
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 3, 1024);
        let csv = snap.format_csv_companion();
        assert!(csv.contains("record,thermal_pressure,detected_agents"));
        assert!(csv.contains("gate,GREEN,3,1024"));
    }

    #[test]
    fn gate_status_snapshot_matches_section() {
        let warn_at = AgentContentionThresholds::default().warn_at;
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Yellow, warn_at, 500);
        assert_eq!(snap.thermal_pressure, "YELLOW");
        assert_eq!(snap.detected_agents, warn_at);
        assert_eq!(snap.agent_contention, "WARN");
        assert_eq!(snap.gate_decision, "ADMIT");
    }

    /// Thresholds are operator-configurable; invalid values fall back to defaults.
    #[test]
    fn contention_threshold_overrides_resolve_and_validate() {
        let tuned = AgentContentionThresholds::from_overrides(Some(20), Some(40));
        assert_eq!((tuned.warn_at, tuned.refuse_at), (20, 40));
        assert_eq!(
            AgentContentionThresholds::from_overrides(None, None),
            AgentContentionThresholds::default()
        );
        assert_eq!(
            AgentContentionThresholds::from_overrides(Some(40), Some(20)),
            AgentContentionThresholds::default(),
            "warn_at >= refuse_at MUST fall back to defaults"
        );
        assert_eq!(
            AgentContentionThresholds::from_overrides(Some(4), Some(0)),
            AgentContentionThresholds::default(),
            "zero refuse_at MUST fall back to defaults"
        );
    }

    /// RSS thresholds are operator-configurable; invalid values fall back.
    #[test]
    fn rss_threshold_overrides_resolve_and_validate() {
        let tuned = AgentResourceThresholds::from_overrides(Some(8_000), Some(16_000));
        assert_eq!((tuned.warn_total_rss_bytes, tuned.refuse_total_rss_bytes), (8_000, 16_000));
        assert_eq!(
            AgentResourceThresholds::from_overrides(Some(16_000), Some(8_000)),
            AgentResourceThresholds::default(),
            "warn >= refuse MUST fall back to defaults"
        );
    }
}
