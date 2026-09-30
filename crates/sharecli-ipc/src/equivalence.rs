//! Adapter-bounded work-equivalence contract.
//!
//! Unknown work bypasses sharing. This module does not replace the current
//! Hypervisor/cache implementation; it is the additive SC-WP-A04 interface.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceEvidence {
    pub adapter_id: String,
    pub adapter_version: String,
    pub input_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquivalenceDecision {
    Bypass,
    InFlight { key: String },
    Durable { key: String, evidence: EquivalenceEvidence },
}

pub trait EquivalenceAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn decide(&self, argv: &[String], cwd: &str) -> EquivalenceDecision;
}

pub struct UnknownAdapter;
impl EquivalenceAdapter for UnknownAdapter {
    fn id(&self) -> &str { "unknown" }
    fn version(&self) -> &str { "0" }
    fn decide(&self, _argv: &[String], _cwd: &str) -> EquivalenceDecision {
        EquivalenceDecision::Bypass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_work_bypasses() {
        let a = UnknownAdapter;
        assert_eq!(a.decide(&["tool".into()], "/tmp"), EquivalenceDecision::Bypass);
    }
}
