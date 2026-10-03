//! Mature-recovery semantic identity types.
//!
//! Additive only: these types do not change current execution behavior.
//! They exist to prevent mature identities from collapsing into PID/string
//! aliases while Tier-B integration remains gated.

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }
    };
}

string_id!(PolicyScopeId);
string_id!(PolicyRevision);
string_id!(PolicyDecisionId);
string_id!(OwnershipClaimId);
string_id!(InvocationId);
string_id!(ExecutionAttemptId);
string_id!(EquivalenceAdapterId);
string_id!(RecoveryOperationId);
string_id!(EvidenceReceiptId);

/// Identity for one particular process lifetime.
///
/// PID is deliberately only one attribute. The provider-defined generation
/// MUST differ when a numeric PID is reused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessGenerationId {
    pub provider: String,
    pub pid: u32,
    pub generation: String,
}

impl ProcessGenerationId {
    pub fn new(provider: impl Into<String>, pid: u32, generation: impl Into<String>) -> Self {
        Self { provider: provider.into(), pid, generation: generation.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_pid_different_generation_is_not_same_process() {
        let old = ProcessGenerationId::new("fixture", 4242, "start-A");
        let reused = ProcessGenerationId::new("fixture", 4242, "start-B");
        assert_ne!(old, reused);
    }

    #[test]
    fn semantic_ids_round_trip() {
        let id = RecoveryOperationId::new("recovery-1");
        let json = serde_json::to_string(&id).unwrap();
        let restored: RecoveryOperationId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }
}
