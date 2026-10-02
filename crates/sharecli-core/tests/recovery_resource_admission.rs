#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use sharecli_core::{
    AdmissionRejection, FakeThermalGate, Hypervisor, HypervisorConfig, ResourceAdmissionPool,
    SpawnRequest, ThermalDecision,
};
use sharecli_ipc::scheduling::{ResourceEnvelope, ResourceVector};
use sharecli_ipc::CacheKeyMode;
use tempfile::TempDir;

fn vector(cpu: f64, memory: u64) -> ResourceVector {
    ResourceVector {
        cpu: Some(cpu),
        memory_bytes: Some(memory),
        gpu_count: Some(0),
        vram_bytes: Some(0),
        disk_bytes: Some(0),
        io_weight: Some(0.0),
        capabilities: vec![],
    }
}

fn hypervisor(dir: &TempDir) -> Hypervisor {
    Hypervisor::from_config_with_gate(
        HypervisorConfig {
            cache_root: dir.path().join("cache"),
            queue_root: dir.path().join("queue"),
            queue_max_concurrent: 8,
            coalesce_ttl: Duration::from_secs(60),
            coalesce_debounce: Duration::ZERO,
            cache_key_mode: CacheKeyMode::Time,
            semantic: false,
        },
        Arc::new(FakeThermalGate::new(ThermalDecision::Allow)),
    )
}

#[tokio::test]
async fn admitted_spawn_holds_and_returns_real_resource_lease() {
    let dir = TempDir::new().expect("tempdir");
    let hv = hypervisor(&dir);
    let pool = ResourceAdmissionPool::new(&ResourceEnvelope {
        id: "host".into(),
        observed_at_unix_ms: 1,
        source: "fixture".into(),
        capacity: vector(1.0, 2_000),
    })
    .expect("known envelope");

    let req = SpawnRequest::new(
        vec!["sh".into(), "-c".into(), "sleep 0.20; printf done".into()],
        std::env::current_dir().expect("cwd"),
        vec![],
    );
    let demand = vector(1.0, 2_000);

    let running = hv.run_admitted_queued(
        req,
        "resource-admission-fixture",
        &pool,
        "work-a",
        &demand,
    );

    let observe_while_running = async {
        for _ in 0..100 {
            if pool.active_count() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert_eq!(pool.active_count(), 1, "spawn never acquired admission lease");
        assert!(matches!(
            pool.try_acquire("work-b", &demand),
            Err(AdmissionRejection::WouldExceed(_))
        ));
    };

    let (outcome, ()) = tokio::join!(running, observe_while_running);
    let outcome = outcome.expect("admitted spawn");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.stdout, b"done");
    assert_eq!(pool.active_count(), 0, "lease was not returned after child exit");

    let next = pool.try_acquire("work-b", &demand).expect("capacity returned");
    assert!(next.reservation_id().contains("work-b"));
}

#[tokio::test]
async fn rejected_admission_never_spawns_process() {
    let dir = TempDir::new().expect("tempdir");
    let marker = dir.path().join("spawned");
    let hv = hypervisor(&dir);
    let pool = ResourceAdmissionPool::new(&ResourceEnvelope {
        id: "host".into(),
        observed_at_unix_ms: 1,
        source: "fixture".into(),
        capacity: vector(1.0, 1_000),
    })
    .expect("known envelope");

    let req = SpawnRequest::new(
        vec![
            "sh".into(),
            "-c".into(),
            format!("touch '{}'", marker.display()),
        ],
        std::env::current_dir().expect("cwd"),
        vec![],
    );

    let result = hv
        .run_admitted_queued(
            req,
            "resource-admission-fixture",
            &pool,
            "too-large",
            &vector(2.0, 2_000),
        )
        .await;

    assert!(result.is_err());
    assert!(!marker.exists(), "rejected work reached subprocess execution");
}
