//! Product-level capability truth projection.
//!
//! Facts are independent: observation never implies ownership, mediation,
//! optimization eligibility, sharing, or recovery management.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityFact {
    pub available: bool,
    pub degraded_reason: Option<String>,
    pub evidence_ref: Option<String>,
}

impl CapabilityFact {
    pub fn no() -> Self { Self { available: false, degraded_reason: None, evidence_ref: None } }
    pub fn yes(evidence_ref: impl Into<String>) -> Self {
        Self { available: true, degraded_reason: None, evidence_ref: Some(evidence_ref.into()) }
    }
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self { available: false, degraded_reason: Some(reason.into()), evidence_ref: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityTruth {
    pub observed: CapabilityFact,
    pub attributed: CapabilityFact,
    pub owned: CapabilityFact,
    pub supervised: CapabilityFact,
    pub mediated: CapabilityFact,
    pub filesystem_intercepted: CapabilityFact,
    pub optimization_eligible: CapabilityFact,
    pub result_shareable: CapabilityFact,
    pub recovery_managed: CapabilityFact,
}

impl CapabilityTruth {
    pub fn observed_only(evidence_ref: impl Into<String>) -> Self {
        Self {
            observed: CapabilityFact::yes(evidence_ref),
            attributed: CapabilityFact::no(), owned: CapabilityFact::no(),
            supervised: CapabilityFact::no(), mediated: CapabilityFact::no(),
            filesystem_intercepted: CapabilityFact::no(),
            optimization_eligible: CapabilityFact::no(),
            result_shareable: CapabilityFact::no(),
            recovery_managed: CapabilityFact::no(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observation_does_not_imply_control_or_optimization() {
        let c = CapabilityTruth::observed_only("obs-1");
        assert!(c.observed.available);
        assert!(!c.owned.available);
        assert!(!c.mediated.available);
        assert!(!c.optimization_eligible.available);
        assert!(!c.result_shareable.available);
    }
}
