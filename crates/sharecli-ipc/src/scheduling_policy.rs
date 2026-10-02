//! Deterministic scheduling-policy primitives for mature-recovery B05.
//!
//! Reference/oracle code only. It is not wired into production execution.

use std::collections::BTreeSet;

use crate::scheduling::{ResourceEnvelope, ResourceVector, WorkItem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyReadiness {
    Ready,
    Blocked(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceFeasibility {
    Fits,
    Unknown(String),
    DoesNotFit(String),
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub item: WorkItem,
    pub demand: ResourceVector,
    pub enqueued_at_ms: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub work_item_id: String,
    pub earliest_start_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackfillEligibility {
    Eligible,
    DependencyBlocked,
    ResourceUnknown,
    ResourceDoesNotFit,
    DurationUnknown,
    DurationProvenanceMissing,
    WouldDelayReservation,
}

pub fn dependency_readiness(
    item: &WorkItem,
    completed: &BTreeSet<String>,
) -> DependencyReadiness {
    let blocked: Vec<String> = item
        .dependencies
        .iter()
        .filter(|id| !completed.contains(*id))
        .cloned()
        .collect();
    if blocked.is_empty() {
        DependencyReadiness::Ready
    } else {
        DependencyReadiness::Blocked(blocked)
    }
}

fn scalar_f64(name: &str, demand: Option<f64>, capacity: Option<f64>) -> ResourceFeasibility {
    let Some(demand) = demand else {
        return ResourceFeasibility::Unknown(format!("{name} demand unknown"));
    };
    if !demand.is_finite() || demand < 0.0 {
        return ResourceFeasibility::Unknown(format!("{name} demand invalid"));
    }
    if demand == 0.0 {
        return ResourceFeasibility::Fits;
    }
    let Some(capacity) = capacity else {
        return ResourceFeasibility::Unknown(format!("{name} capacity unknown"));
    };
    if !capacity.is_finite() || capacity < 0.0 {
        return ResourceFeasibility::Unknown(format!("{name} capacity invalid"));
    }
    if demand <= capacity {
        ResourceFeasibility::Fits
    } else {
        ResourceFeasibility::DoesNotFit(format!("{name} demand exceeds capacity"))
    }
}

fn scalar_u64(name: &str, demand: Option<u64>, capacity: Option<u64>) -> ResourceFeasibility {
    let Some(demand) = demand else {
        return ResourceFeasibility::Unknown(format!("{name} demand unknown"));
    };
    if demand == 0 {
        return ResourceFeasibility::Fits;
    }
    let Some(capacity) = capacity else {
        return ResourceFeasibility::Unknown(format!("{name} capacity unknown"));
    };
    if demand <= capacity {
        ResourceFeasibility::Fits
    } else {
        ResourceFeasibility::DoesNotFit(format!("{name} demand exceeds capacity"))
    }
}

fn scalar_u32(name: &str, demand: Option<u32>, capacity: Option<u32>) -> ResourceFeasibility {
    let Some(demand) = demand else {
        return ResourceFeasibility::Unknown(format!("{name} demand unknown"));
    };
    if demand == 0 {
        return ResourceFeasibility::Fits;
    }
    let Some(capacity) = capacity else {
        return ResourceFeasibility::Unknown(format!("{name} capacity unknown"));
    };
    if demand <= capacity {
        ResourceFeasibility::Fits
    } else {
        ResourceFeasibility::DoesNotFit(format!("{name} demand exceeds capacity"))
    }
}

pub fn resource_feasibility(
    envelope: &ResourceEnvelope,
    demand: &ResourceVector,
) -> ResourceFeasibility {
    for result in [
        scalar_f64("cpu", demand.cpu, envelope.capacity.cpu),
        scalar_u64("memory", demand.memory_bytes, envelope.capacity.memory_bytes),
        scalar_u32("gpu_count", demand.gpu_count, envelope.capacity.gpu_count),
        scalar_u64("vram", demand.vram_bytes, envelope.capacity.vram_bytes),
        scalar_u64("disk", demand.disk_bytes, envelope.capacity.disk_bytes),
        scalar_f64("io_weight", demand.io_weight, envelope.capacity.io_weight),
    ] {
        if result != ResourceFeasibility::Fits {
            return result;
        }
    }

    for capability in &demand.capabilities {
        if !envelope.capacity.capabilities.iter().any(|c| c == capability) {
            return ResourceFeasibility::DoesNotFit(format!(
                "missing required capability {capability:?}"
            ));
        }
    }

    ResourceFeasibility::Fits
}

pub fn effective_priority(
    item: &WorkItem,
    enqueued_at_ms: u64,
    now_ms: u64,
    aging_interval_ms: u64,
) -> i128 {
    let age_steps = if aging_interval_ms == 0 {
        0
    } else {
        now_ms.saturating_sub(enqueued_at_ms) / aging_interval_ms
    };
    i128::from(item.priority) + i128::from(age_steps)
}

/// Deterministic queue order: effective priority desc, enqueue time asc,
/// explicit sequence asc. PID and lexical work-item ID never participate.
pub fn ranked_ready_ids(
    entries: &[QueueEntry],
    completed: &BTreeSet<String>,
    now_ms: u64,
    aging_interval_ms: u64,
) -> Vec<String> {
    let mut ready: Vec<&QueueEntry> = entries
        .iter()
        .filter(|entry| dependency_readiness(&entry.item, completed) == DependencyReadiness::Ready)
        .collect();

    ready.sort_by(|a, b| {
        effective_priority(&b.item, b.enqueued_at_ms, now_ms, aging_interval_ms)
            .cmp(&effective_priority(&a.item, a.enqueued_at_ms, now_ms, aging_interval_ms))
            .then_with(|| a.enqueued_at_ms.cmp(&b.enqueued_at_ms))
            .then_with(|| a.sequence.cmp(&b.sequence))
    });

    ready.into_iter().map(|entry| entry.item.id.clone()).collect()
}

pub fn conservative_backfill_eligibility(
    entry: &QueueEntry,
    envelope: &ResourceEnvelope,
    completed: &BTreeSet<String>,
    now_ms: u64,
    reservation: &Reservation,
) -> BackfillEligibility {
    if dependency_readiness(&entry.item, completed) != DependencyReadiness::Ready {
        return BackfillEligibility::DependencyBlocked;
    }

    match resource_feasibility(envelope, &entry.demand) {
        ResourceFeasibility::Unknown(_) => return BackfillEligibility::ResourceUnknown,
        ResourceFeasibility::DoesNotFit(_) => return BackfillEligibility::ResourceDoesNotFit,
        ResourceFeasibility::Fits => {}
    }

    let Some(duration_ms) = entry.item.estimated_duration_ms else {
        return BackfillEligibility::DurationUnknown;
    };
    if entry.item.estimate_provenance.as_deref().unwrap_or("").is_empty() {
        return BackfillEligibility::DurationProvenanceMissing;
    }

    if now_ms.saturating_add(duration_ms) > reservation.earliest_start_ms {
        return BackfillEligibility::WouldDelayReservation;
    }

    BackfillEligibility::Eligible
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn rv(
        cpu: f64,
        mem: u64,
        gpu: u32,
        vram: u64,
        caps: &[&str],
    ) -> ResourceVector {
        ResourceVector {
            cpu: Some(cpu),
            memory_bytes: Some(mem),
            gpu_count: Some(gpu),
            vram_bytes: Some(vram),
            disk_bytes: Some(0),
            io_weight: Some(0.0),
            capabilities: caps.iter().map(|x| (*x).to_string()).collect(),
        }
    }

    fn work(id: &str, deps: &[&str], priority: i64, duration_ms: Option<u64>) -> WorkItem {
        WorkItem {
            id: id.into(),
            dependencies: deps.iter().map(|x| (*x).to_string()).collect(),
            priority,
            estimated_duration_ms: duration_ms,
            estimate_provenance: duration_ms.map(|_| "fixture-profile:v1".into()),
        }
    }

    #[test]
    fn dependencies_block_until_all_predecessors_complete() {
        let item = work("link", &["compile-a", "compile-b"], 0, Some(10));
        let mut completed = BTreeSet::new();
        completed.insert("compile-a".into());
        assert_eq!(
            dependency_readiness(&item, &completed),
            DependencyReadiness::Blocked(vec!["compile-b".into()])
        );
        completed.insert("compile-b".into());
        assert_eq!(dependency_readiness(&item, &completed), DependencyReadiness::Ready);
    }

    #[test]
    fn gpu_vram_and_capabilities_are_hard_feasibility_dimensions() {
        let env = ResourceEnvelope {
            id: "gpu-host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: rv(8.0, 64_000, 1, 24_000, &["cuda", "sm_89"]),
        };
        assert_eq!(
            resource_feasibility(&env, &rv(2.0, 8_000, 1, 20_000, &["cuda"])),
            ResourceFeasibility::Fits
        );
        assert!(matches!(
            resource_feasibility(&env, &rv(2.0, 8_000, 2, 20_000, &["cuda"])),
            ResourceFeasibility::DoesNotFit(_)
        ));
        assert!(matches!(
            resource_feasibility(&env, &rv(2.0, 8_000, 1, 20_000, &["rocm"])),
            ResourceFeasibility::DoesNotFit(_)
        ));
    }

    #[test]
    fn invalid_float_demand_or_capacity_is_not_schedulable() {
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: rv(8.0, 64_000, 0, 0, &[]),
        };

        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            let mut demand = rv(1.0, 1_000, 0, 0, &[]);
            demand.cpu = Some(invalid);
            assert!(matches!(
                resource_feasibility(&env, &demand),
                ResourceFeasibility::Unknown(_)
            ));

            let mut invalid_env = env.clone();
            invalid_env.capacity.cpu = Some(invalid);
            assert!(matches!(
                resource_feasibility(&invalid_env, &rv(1.0, 1_000, 0, 0, &[])),
                ResourceFeasibility::Unknown(_)
            ));
        }
    }

    #[test]
    fn unknown_device_demand_is_not_zero() {
        let env = ResourceEnvelope {
            id: "gpu-host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: rv(8.0, 64_000, 1, 24_000, &["cuda"]),
        };
        let mut d = rv(1.0, 1_000, 0, 0, &[]);
        d.gpu_count = None;
        assert!(matches!(
            resource_feasibility(&env, &d),
            ResourceFeasibility::Unknown(_)
        ));
    }

    #[test]
    fn aging_is_deterministic_and_does_not_use_id_or_pid_tiebreaks() {
        let entries = vec![
            QueueEntry {
                item: work("z-older", &[], 0, Some(10)),
                demand: rv(1.0, 1, 0, 0, &[]),
                enqueued_at_ms: 0,
                sequence: 1,
            },
            QueueEntry {
                item: work("a-newer-high-priority", &[], 2, Some(10)),
                demand: rv(1.0, 1, 0, 0, &[]),
                enqueued_at_ms: 150,
                sequence: 2,
            },
        ];
        let ids = ranked_ready_ids(&entries, &BTreeSet::new(), 300, 100);
        assert_eq!(ids, vec!["z-older", "a-newer-high-priority"]);
    }

    #[test]
    fn dependency_blocked_high_priority_work_cannot_jump_readiness() {
        let entries = vec![
            QueueEntry {
                item: work("blocked", &["missing"], 100, Some(10)),
                demand: rv(1.0, 1, 0, 0, &[]),
                enqueued_at_ms: 0,
                sequence: 1,
            },
            QueueEntry {
                item: work("ready", &[], 0, Some(10)),
                demand: rv(1.0, 1, 0, 0, &[]),
                enqueued_at_ms: 1,
                sequence: 2,
            },
        ];
        assert_eq!(
            ranked_ready_ids(&entries, &BTreeSet::new(), 10, 100),
            vec!["ready"]
        );
    }

    #[test]
    fn conservative_backfill_cannot_delay_reserved_head() {
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: rv(4.0, 8_000, 0, 0, &[]),
        };
        let reservation = Reservation {
            work_item_id: "head".into(),
            earliest_start_ms: 100,
        };
        let short = QueueEntry {
            item: work("short", &[], 0, Some(40)),
            demand: rv(1.0, 1_000, 0, 0, &[]),
            enqueued_at_ms: 0,
            sequence: 1,
        };
        let long = QueueEntry {
            item: work("long", &[], 0, Some(101)),
            demand: rv(1.0, 1_000, 0, 0, &[]),
            enqueued_at_ms: 0,
            sequence: 2,
        };
        assert_eq!(
            conservative_backfill_eligibility(
                &short,
                &env,
                &BTreeSet::new(),
                0,
                &reservation
            ),
            BackfillEligibility::Eligible
        );
        assert_eq!(
            conservative_backfill_eligibility(
                &long,
                &env,
                &BTreeSet::new(),
                0,
                &reservation
            ),
            BackfillEligibility::WouldDelayReservation
        );
    }

    #[test]
    fn backfill_requires_duration_provenance() {
        let env = ResourceEnvelope {
            id: "host".into(),
            observed_at_unix_ms: 1,
            source: "fixture".into(),
            capacity: rv(4.0, 8_000, 0, 0, &[]),
        };
        let reservation = Reservation {
            work_item_id: "head".into(),
            earliest_start_ms: 100,
        };
        let mut item = work("candidate", &[], 0, Some(10));
        item.estimate_provenance = None;
        let entry = QueueEntry {
            item,
            demand: rv(1.0, 1_000, 0, 0, &[]),
            enqueued_at_ms: 0,
            sequence: 1,
        };
        assert_eq!(
            conservative_backfill_eligibility(
                &entry,
                &env,
                &BTreeSet::new(),
                0,
                &reservation
            ),
            BackfillEligibility::DurationProvenanceMissing
        );
    }

    #[test]
    fn planner_latency_is_measured_without_inventing_a_pass_threshold() {
        let entries: Vec<QueueEntry> = (0..1_000)
            .map(|i| QueueEntry {
                item: work(&format!("w-{i}"), &[], (i % 5) as i64, Some(10)),
                demand: rv(1.0, 1, 0, 0, &[]),
                enqueued_at_ms: i as u64,
                sequence: i as u64,
            })
            .collect();
        let started = Instant::now();
        let ids = ranked_ready_ids(&entries, &BTreeSet::new(), 5_000, 100);
        let elapsed = started.elapsed();
        assert_eq!(ids.len(), entries.len());
        eprintln!(
            "sharecli_reference_planner items={} elapsed_ns={}",
            entries.len(),
            elapsed.as_nanos()
        );
    }
}
