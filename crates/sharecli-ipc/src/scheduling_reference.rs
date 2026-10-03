//! Deterministic reference scheduler for the v1.2 recovery program.
//! Pure model only; not wired into current ShareCLI execution.

use crate::scheduling::{
    PlannedPlacement, ResourceEnvelope, ResourceVector, ScheduleDecision, SchedulePlan, WorkItem,
};
use crate::scheduling_policy::{resource_feasibility, ResourceFeasibility};

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
    pub admitted_gpu_count: u32,
    pub admitted_vram_bytes: u64,
    pub admitted_disk_bytes: u64,
    pub admitted_io_weight: f64,
    pub queued: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct UsedResources {
    cpu: f64,
    memory_bytes: u64,
    gpu_count: u32,
    vram_bytes: u64,
    disk_bytes: u64,
    io_weight: f64,
}

fn scalar_demands(demand: &ResourceVector) -> Option<UsedResources> {
    Some(UsedResources {
        cpu: demand.cpu?,
        memory_bytes: demand.memory_bytes?,
        gpu_count: demand.gpu_count?,
        vram_bytes: demand.vram_bytes?,
        disk_bytes: demand.disk_bytes?,
        io_weight: demand.io_weight?,
    })
}

fn fits_remaining(
    used: UsedResources,
    demand: UsedResources,
    capacity: &ResourceVector,
) -> bool {
    used.cpu + demand.cpu <= capacity.cpu.unwrap_or(0.0)
        && used.memory_bytes.saturating_add(demand.memory_bytes)
            <= capacity.memory_bytes.unwrap_or(0)
        && used.gpu_count.saturating_add(demand.gpu_count)
            <= capacity.gpu_count.unwrap_or(0)
        && used.vram_bytes.saturating_add(demand.vram_bytes)
            <= capacity.vram_bytes.unwrap_or(0)
        && used.disk_bytes.saturating_add(demand.disk_bytes)
            <= capacity.disk_bytes.unwrap_or(0)
        && used.io_weight + demand.io_weight <= capacity.io_weight.unwrap_or(0.0)
}

fn add_resources(used: &mut UsedResources, demand: UsedResources) {
    used.cpu += demand.cpu;
    used.memory_bytes = used.memory_bytes.saturating_add(demand.memory_bytes);
    used.gpu_count = used.gpu_count.saturating_add(demand.gpu_count);
    used.vram_bytes = used.vram_bytes.saturating_add(demand.vram_bytes);
    used.disk_bytes = used.disk_bytes.saturating_add(demand.disk_bytes);
    used.io_weight += demand.io_weight;
}

pub fn bounded_fifo_pack(
    plan_id: impl Into<String>,
    policy_revision: impl Into<String>,
    envelope: &ResourceEnvelope,
    work: &[SchedulableWork],
) -> PackingReceipt {
    let mut used = UsedResources::default();
    let mut queued = 0usize;
    let mut placements = Vec::with_capacity(work.len());

    for work_item in work {
        match resource_feasibility(envelope, &work_item.demand) {
            ResourceFeasibility::Unknown(reason) => {
                queued += 1;
                placements.push(PlannedPlacement {
                    work_item_id: work_item.item.id.clone(),
                    decision: ScheduleDecision::Defer,
                    target: None,
                    reason,
                });
                continue;
            }
            ResourceFeasibility::DoesNotFit(reason) => {
                queued += 1;
                placements.push(PlannedPlacement {
                    work_item_id: work_item.item.id.clone(),
                    decision: ScheduleDecision::Defer,
                    target: None,
                    reason,
                });
                continue;
            }
            ResourceFeasibility::Fits => {}
        }

        let Some(demand) = scalar_demands(&work_item.demand) else {
            queued += 1;
            placements.push(PlannedPlacement {
                work_item_id: work_item.item.id.clone(),
                decision: ScheduleDecision::Defer,
                target: None,
                reason: "one or more scalar resource demands are unknown".into(),
            });
            continue;
        };

        if fits_remaining(used, demand, &envelope.capacity) {
            add_resources(&mut used, demand);
            placements.push(PlannedPlacement {
                work_item_id: work_item.item.id.clone(),
                decision: ScheduleDecision::Admit,
                target: Some(envelope.id.clone()),
                reason: "fits remaining multidimensional envelope".into(),
            });
        } else {
            queued += 1;
            placements.push(PlannedPlacement {
                work_item_id: work_item.item.id.clone(),
                decision: ScheduleDecision::Queue,
                target: None,
                reason: "would exceed remaining multidimensional envelope".into(),
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
        admitted_cpu: used.cpu,
        admitted_memory_bytes: used.memory_bytes,
        admitted_gpu_count: used.gpu_count,
        admitted_vram_bytes: used.vram_bytes,
        admitted_disk_bytes: used.disk_bytes,
        admitted_io_weight: used.io_weight,
        queued,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(cpu: f64, mem: u64) -> ResourceVector {
        ResourceVector {
            cpu: Some(cpu),
            memory_bytes: Some(mem),
            gpu_count: Some(0),
            vram_bytes: Some(0),
            disk_bytes: Some(0),
            io_weight: Some(0.0),
            capabilities: vec![],
        }
    }

    fn item(id: &str) -> WorkItem {
        WorkItem {
            id: id.into(),
            dependencies: vec![],
            priority: 0,
            estimated_duration_ms: None,
            estimate_provenance: None,
        }
    }

    #[test]
    fn pack_never_exceeds_hard_envelope() {
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: demand(4.0, 8_000),
        };
        let work = vec![
            SchedulableWork { item: item("a"), demand: demand(2.0, 4_000) },
            SchedulableWork { item: item("b"), demand: demand(2.0, 4_000) },
            SchedulableWork { item: item("c"), demand: demand(2.0, 4_000) },
        ];
        let receipt = bounded_fifo_pack("p", "fifo-v1", &env, &work);
        assert_eq!(receipt.queued, 1);
        assert!(receipt.admitted_cpu <= 4.0);
        assert!(receipt.admitted_memory_bytes <= 8_000);
    }

    #[test]
    fn unknown_demand_is_deferred_not_zero() {
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: demand(4.0, 8_000),
        };
        let mut unknown = demand(1.0, 1_000);
        unknown.memory_bytes = None;
        let receipt = bounded_fifo_pack(
            "p",
            "fifo-v1",
            &env,
            &[SchedulableWork { item: item("unknown"), demand: unknown }],
        );
        assert_eq!(receipt.plan.placements[0].decision, ScheduleDecision::Defer);
    }

    #[test]
    fn gpu_and_vram_are_accumulated_not_merely_checked_per_item() {
        let mut capacity = demand(8.0, 64_000);
        capacity.gpu_count = Some(1);
        capacity.vram_bytes = Some(24_000);
        capacity.capabilities = vec!["cuda".into()];
        let env = ResourceEnvelope {
            id: "gpu-host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity,
        };

        let mut first = demand(1.0, 4_000);
        first.gpu_count = Some(1);
        first.vram_bytes = Some(10_000);
        first.capabilities = vec!["cuda".into()];

        let mut second = demand(1.0, 4_000);
        second.gpu_count = Some(1);
        second.vram_bytes = Some(10_000);
        second.capabilities = vec!["cuda".into()];

        let receipt = bounded_fifo_pack(
            "p",
            "gpu-v1",
            &env,
            &[
                SchedulableWork { item: item("gpu-a"), demand: first },
                SchedulableWork { item: item("gpu-b"), demand: second },
            ],
        );
        assert_eq!(receipt.plan.placements[0].decision, ScheduleDecision::Admit);
        assert_eq!(receipt.plan.placements[1].decision, ScheduleDecision::Queue);
        assert_eq!(receipt.admitted_gpu_count, 1);
        assert_eq!(receipt.admitted_vram_bytes, 10_000);
    }

    #[test]
    fn missing_required_capability_is_deferred() {
        let mut capacity = demand(8.0, 64_000);
        capacity.capabilities = vec!["cuda".into()];
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity,
        };
        let mut work = demand(1.0, 1_000);
        work.capabilities = vec!["rocm".into()];

        let receipt = bounded_fifo_pack(
            "p",
            "capability-v1",
            &env,
            &[SchedulableWork { item: item("rocm-work"), demand: work }],
        );
        assert_eq!(receipt.plan.placements[0].decision, ScheduleDecision::Defer);
    }

    #[test]
    fn disk_and_io_are_accumulated_when_declared() {
        let mut capacity = demand(8.0, 64_000);
        capacity.disk_bytes = Some(10_000);
        capacity.io_weight = Some(1.0);
        let env = ResourceEnvelope {
            id: "io-host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity,
        };

        let mut first = demand(1.0, 1_000);
        first.disk_bytes = Some(6_000);
        first.io_weight = Some(0.6);
        let mut second = demand(1.0, 1_000);
        second.disk_bytes = Some(6_000);
        second.io_weight = Some(0.6);

        let receipt = bounded_fifo_pack(
            "p",
            "io-v1",
            &env,
            &[
                SchedulableWork { item: item("io-a"), demand: first },
                SchedulableWork { item: item("io-b"), demand: second },
            ],
        );
        assert_eq!(receipt.plan.placements[0].decision, ScheduleDecision::Admit);
        assert_eq!(receipt.plan.placements[1].decision, ScheduleDecision::Queue);
        assert_eq!(receipt.admitted_disk_bytes, 6_000);
        assert_eq!(receipt.admitted_io_weight, 0.6);
    }
}
