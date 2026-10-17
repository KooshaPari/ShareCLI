//! Additive workload/resource scheduling domain for the v1.2 ShareCLI thesis.
//! No existing scheduler/runtime path consumes these types yet.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceVector {
    pub cpu: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub gpu_count: Option<u32>,
    pub vram_bytes: Option<u64>,
    pub disk_bytes: Option<u64>,
    pub io_weight: Option<f64>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl ResourceVector {
    pub fn unknown() -> Self {
        Self { cpu: None, memory_bytes: None, gpu_count: None, vram_bytes: None, disk_bytes: None, io_weight: None, capabilities: vec![] }
    }

    pub fn has_unknown_scalar(&self) -> bool {
        self.cpu.is_none()
            || self.memory_bytes.is_none()
            || self.gpu_count.is_none()
            || self.vram_bytes.is_none()
            || self.disk_bytes.is_none()
            || self.io_weight.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub dependencies: Vec<String>,
    pub priority: i64,
    pub estimated_duration_ms: Option<u64>,
    pub estimate_provenance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceEnvelope {
    pub id: String,
    pub observed_at_unix_ms: i64,
    pub source: String,
    pub capacity: ResourceVector,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleDecision {
    Admit,
    Queue,
    Defer,
    Refuse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPlacement {
    pub work_item_id: String,
    pub decision: ScheduleDecision,
    pub target: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulePlan {
    pub id: String,
    pub policy_revision: String,
    pub envelope_id: String,
    pub placements: Vec<PlannedPlacement>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_resource_demand_is_not_zero() {
        let r = ResourceVector::unknown();
        assert!(r.cpu.is_none());
        assert!(r.memory_bytes.is_none());
        assert!(r.gpu_count.is_none());
        assert!(r.vram_bytes.is_none());
        assert!(r.disk_bytes.is_none());
        assert!(r.io_weight.is_none());
        assert!(r.has_unknown_scalar());
    }

    #[test]
    fn plan_is_only_a_proposal() {
        let p = SchedulePlan {
            id: "plan-1".into(),
            policy_revision: "policy-1".into(),
            envelope_id: "env-1".into(),
            placements: vec![PlannedPlacement {
                work_item_id: "w1".into(),
                decision: ScheduleDecision::Queue,
                target: None,
                reason: "insufficient memory".into(),
            }],
        };
        assert_eq!(p.placements[0].decision, ScheduleDecision::Queue);
    }

    #[test]
    fn duration_estimate_requires_separate_provenance_field() {
        let w = WorkItem {
            id: "w1".into(),
            dependencies: vec![],
            priority: 0,
            estimated_duration_ms: Some(5000),
            estimate_provenance: Some("historical-profile:v1".into()),
        };
        assert!(w.estimated_duration_ms.is_some());
        assert!(w.estimate_provenance.is_some());
    }
}

#[cfg(test)]
mod fixture_scheduler;
