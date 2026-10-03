//! Resource-control capability contract.
//!
//! Observation and enforcement are independent. A pressure/usage provider does
//! not imply any control capability.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceControlCapabilities {
    pub provider_id: String,
    pub cpu_weight: bool,
    pub cpu_hard_cap: bool,
    pub memory_hard_limit: bool,
    pub pause_resume: bool,
    pub terminate: bool,
    pub gpu_partition: bool,
}

impl ResourceControlCapabilities {
    pub fn observe_only(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            cpu_weight: false,
            cpu_hard_cap: false,
            memory_hard_limit: false,
            pause_resume: false,
            terminate: false,
            gpu_partition: false,
        }
    }

    pub fn can_enforce_anything(&self) -> bool {
        self.cpu_weight
            || self.cpu_hard_cap
            || self.memory_hard_limit
            || self.pause_resume
            || self.terminate
            || self.gpu_partition
    }
}

pub trait ResourceControlProvider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ResourceControlCapabilities;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_only_provider_has_no_enforcement_truth() {
        let c = ResourceControlCapabilities::observe_only("linux-psi");
        assert!(!c.can_enforce_anything());
        assert!(!c.cpu_hard_cap);
        assert!(!c.memory_hard_limit);
    }

    #[test]
    fn enforcement_truth_is_capability_specific() {
        let c = ResourceControlCapabilities {
            provider_id: "windows-job-object".into(),
            cpu_weight: true,
            cpu_hard_cap: true,
            memory_hard_limit: false,
            pause_resume: false,
            terminate: true,
            gpu_partition: false,
        };
        assert!(c.can_enforce_anything());
        assert!(c.cpu_hard_cap);
        assert!(!c.memory_hard_limit);
        assert!(!c.gpu_partition);
    }
}
