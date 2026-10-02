#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use sharecli_core::{
    FakeThermalGate, Hypervisor, HypervisorConfig, ResourceAdmissionPool, SpawnRequest,
    ThermalDecision,
};
use sharecli_ipc::scheduling::{ResourceEnvelope, ResourceVector, ScheduleDecision, WorkItem};
use sharecli_ipc::scheduling_policy::{ranked_ready_ids, QueueEntry};
use sharecli_ipc::scheduling_reference::{bounded_fifo_pack, SchedulableWork};
use sharecli_ipc::{CacheKeyMode, QueuePriority};
use tempfile::TempDir;

fn resources(cpu: f64, memory_bytes: u64) -> ResourceVector {
    ResourceVector {
        cpu: Some(cpu),
        memory_bytes: Some(memory_bytes),
        gpu_count: Some(0),
        vram_bytes: Some(0),
        disk_bytes: Some(0),
        io_weight: Some(0.0),
        capabilities: vec![],
    }
}

fn item(id: &str, dependencies: &[&str]) -> WorkItem {
    WorkItem {
        id: id.into(),
        dependencies: dependencies.iter().map(|value| (*value).to_string()).collect(),
        priority: 0,
        estimated_duration_ms: Some(10),
        estimate_provenance: Some("runtime-fixture:v1".into()),
    }
}

fn hypervisor(dir: &TempDir) -> Hypervisor {
    Hypervisor::from_config_with_gate(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 1,
            coalesce_ttl: Duration::from_secs(60),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
    )
}

fn spawn_for(id: &str, order_file: &std::path::Path) -> SpawnRequest {
    SpawnRequest::new(
        vec![
            "sh".into(),
            "-c".into(),
            r#"printf '%s\n' "$WORK_ID" >> "$ORDER_FILE""#.into(),
        ],
        std::env::current_dir().expect("cwd"),
        vec![
            ("WORK_ID".into(), id.into()),
            ("ORDER_FILE".into(), order_file.display().to_string()),
        ],
    )
    .with_queue_priority(QueuePriority::Normal)
}

#[tokio::test]
async fn planned_dependency_ready_work_reaches_real_hypervisor_execution() {
    let dir = TempDir::new().expect("tempdir");
    let order_file = dir.path().join("order.txt");
    let hv = hypervisor(&dir);

    let entries = vec![
        QueueEntry {
            item: item("compile", &[]),
            demand: resources(1.0, 1_000),
            enqueued_at_ms: 0,
            sequence: 0,
        },
        QueueEntry {
            item: item("link", &["compile"]),
            demand: resources(1.0, 1_000),
            enqueued_at_ms: 1,
            sequence: 1,
        },
        QueueEntry {
            item: item("docs", &[]),
            demand: resources(1.0, 1_000),
            enqueued_at_ms: 2,
            sequence: 2,
        },
    ];
    let by_id: BTreeMap<String, QueueEntry> =
        entries.iter().cloned().map(|entry| (entry.item.id.clone(), entry)).collect();
    let envelope = ResourceEnvelope {
        id: "fixture-host".into(),
        observed_at_unix_ms: 1,
        source: "runtime-fixture".into(),
        capacity: resources(1.0, 2_000),
    };
    let admission = ResourceAdmissionPool::new(&envelope).expect("known runtime envelope");

    let mut completed = BTreeSet::new();
    let mut iterations = 0usize;

    while completed.len() < entries.len() {
        iterations += 1;
        assert!(iterations <= entries.len() + 1, "scheduler made no progress");

        let ready_ids = ranked_ready_ids(&entries, &completed, 10, 1_000);
        let ready: Vec<SchedulableWork> = ready_ids
            .iter()
            .filter(|id| !completed.contains(*id))
            .map(|id| {
                let entry = by_id.get(id).expect("ready entry exists");
                SchedulableWork { item: entry.item.clone(), demand: entry.demand.clone() }
            })
            .collect();

        let receipt = bounded_fifo_pack(
            format!("plan-{iterations}"),
            "runtime-fixture-policy-v1",
            &envelope,
            &ready,
        );
        let admitted: Vec<String> = receipt
            .plan
            .placements
            .iter()
            .filter(|placement| placement.decision == ScheduleDecision::Admit)
            .map(|placement| placement.work_item_id.clone())
            .collect();

        assert!(!admitted.is_empty(), "ready work produced no executable placement");

        for id in admitted {
            let demand = &by_id.get(&id).expect("admitted entry exists").demand;
            let outcome = hv
                .run_admitted_queued(
                    spawn_for(&id, &order_file),
                    "b05-runtime-fixture",
                    &admission,
                    &id,
                    demand,
                )
                .await
                .expect("scheduled Hypervisor execution with runtime admission");
            assert_eq!(outcome.exit_code, 0);
            assert!(!outcome.from_cache);
            assert_eq!(admission.active_count(), 0, "completed work leaked reservation");
            completed.insert(id);
        }
    }

    let order = std::fs::read_to_string(&order_file).expect("execution order");
    let lines: Vec<&str> = order.lines().collect();
    assert_eq!(lines, vec!["compile", "link", "docs"]);
}

#[tokio::test]
async fn unknown_resource_demand_never_reaches_spawn() {
    let dir = TempDir::new().expect("tempdir");
    let order_file = dir.path().join("unknown-order.txt");
    let _hv = hypervisor(&dir);

    let mut demand = resources(1.0, 1_000);
    demand.memory_bytes = None;
    let ready = vec![SchedulableWork { item: item("unknown", &[]), demand }];
    let envelope = ResourceEnvelope {
        id: "fixture-host".into(),
        observed_at_unix_ms: 1,
        source: "runtime-fixture".into(),
        capacity: resources(1.0, 2_000),
    };

    let receipt =
        bounded_fifo_pack("unknown-plan", "runtime-fixture-policy-v1", &envelope, &ready);
    assert_eq!(receipt.plan.placements.len(), 1);
    assert_eq!(receipt.plan.placements[0].decision, ScheduleDecision::Defer);
    assert!(!order_file.exists(), "deferred unknown work reached process execution");
}
