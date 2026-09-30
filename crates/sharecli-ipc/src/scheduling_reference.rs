//! Deterministic reference scheduler for the v1.2 recovery program.
//! Pure model only; not wired into current ShareCLI execution.

use crate::scheduling::{PlannedPlacement, ResourceEnvelope, ResourceVector, ScheduleDecision, SchedulePlan, WorkItem};

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulableWork {
    pub item: WorkItem,
    pub demand: ResourceVector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackingReceipt {
    pub plan: SchedulePlan,
    pub admitted_cpu: f64,
    pub admitted_memory_bytes: u64,
    pub queued: usize,
}

pub fn bounded_fifo_pack(
    plan_id: impl Into<String>,
    policy_revision: impl Into<String>,
    envelope: &ResourceEnvelope,
    work: &[SchedulableWork],
) -> PackingReceipt {
    let cap_cpu = envelope.capacity.cpu.unwrap_or(0.0);
    let cap_mem = envelope.capacity.memory_bytes.unwrap_or(0);
    let mut used_cpu = 0.0;
    let mut used_mem = 0u64;
    let mut queued = 0usize;
    let mut placements = Vec::with_capacity(work.len());

    for w in work {
        let Some(cpu) = w.demand.cpu else {
            queued += 1;
            placements.push(PlannedPlacement {
                work_item_id: w.item.id.clone(),
                decision: ScheduleDecision::Defer,
                target: None,
                reason: "unknown cpu demand".into(),
            });
            continue;
        };
        let Some(mem) = w.demand.memory_bytes else {
            queued += 1;
            placements.push(PlannedPlacement {
                work_item_id: w.item.id.clone(),
                decision: ScheduleDecision::Defer,
                target: None,
                reason: "unknown memory demand".into(),
            });
            continue;
        };

        if used_cpu + cpu <= cap_cpu && used_mem.saturating_add(mem) <= cap_mem {
            used_cpu += cpu;
            used_mem += mem;
            placements.push(PlannedPlacement {
                work_item_id: w.item.id.clone(),
                decision: ScheduleDecision::Admit,
                target: Some(envelope.id.clone()),
                reason: "fits current envelope".into(),
            });
        } else {
            queued += 1;
            placements.push(PlannedPlacement {
                work_item_id: w.item.id.clone(),
                decision: ScheduleDecision::Queue,
                target: None,
                reason: "would exceed current envelope".into(),
            });
        }
    }

    PackingReceipt {
        plan: SchedulePlan {
            id: plan_id.into(),
            policy_revision: policy_revision.into(),
            envelope_id: envelope.id.clone(),
            placements,
        },
        admitted_cpu: used_cpu,
        admitted_memory_bytes: used_mem,
        queued,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(cpu: f64, mem: u64) -> ResourceVector {
        ResourceVector {
            cpu: Some(cpu), memory_bytes: Some(mem), gpu_count: Some(0), vram_bytes: Some(0),
            disk_bytes: None, io_weight: None, capabilities: vec![],
        }
    }

    fn item(id: &str) -> WorkItem {
        WorkItem { id: id.into(), dependencies: vec![], priority: 0, estimated_duration_ms: None, estimate_provenance: None }
    }

    #[test]
    fn pack_never_exceeds_hard_envelope() {
        let env = ResourceEnvelope { id:"host".into(), observed_at_unix_ms:1, source:"fixture".into(), capacity:demand(4.0, 8_000) };
        let work = vec![
            SchedulableWork { item:item("a"), demand:demand(2.0, 4_000) },
            SchedulableWork { item:item("b"), demand:demand(2.0, 4_000) },
            SchedulableWork { item:item("c"), demand:demand(2.0, 4_000) },
        ];
        let r = bounded_fifo_pack("p","fifo-v1",&env,&work);
        assert_eq!(r.queued, 1);
        assert!(r.admitted_cpu <= 4.0);
        assert!(r.admitted_memory_bytes <= 8_000);
    }

    #[test]
    fn unknown_demand_is_deferred_not_zero() {
        let env = ResourceEnvelope { id:"host".into(), observed_at_unix_ms:1, source:"fixture".into(), capacity:demand(4.0, 8_000) };
        let mut u = demand(1.0, 1_000);
        u.memory_bytes = None;
        let r = bounded_fifo_pack("p","fifo-v1",&env,&[SchedulableWork { item:item("u"), demand:u }]);
        assert_eq!(r.plan.placements[0].decision, ScheduleDecision::Defer);
    }
}
