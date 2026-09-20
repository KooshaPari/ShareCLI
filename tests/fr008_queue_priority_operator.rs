//! FR-008 / AC-008.15 — operator queue priority → `SpawnRequest::queue_priority`.
//!
//! AC-008.15: `SHARECLI_QUEUE_PRIORITY` and rules.conf `priority=` MUST propagate
//! into Hypervisor nocache lane priority for harness callers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serial_test::serial;
use sharecli_core::{FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision};
use sharecli_ipc::{resolve_operator_queue_priority, QUEUE_PRIORITY_ENV};
use tempfile::TempDir;

/// FR-008 / AC-008.15 — `SpawnRequest::from_operator` reads `SHARECLI_QUEUE_PRIORITY`.
#[test]
#[serial]
fn fr008_spawn_request_operator_env_priority() {
    unsafe {
        std::env::set_var(QUEUE_PRIORITY_ENV, "critical");
    }
    let req = SpawnRequest::from_operator(
        vec!["ruff".into(), "check".into(), "--fix".into()],
        std::env::current_dir().expect("cwd"),
        vec![],
        Some("low"),
    );
    assert_eq!(
        req.queue_priority,
        QueuePriority::Critical,
        "AC-008.15: env MUST override rules.conf on SpawnRequest"
    );
    unsafe {
        std::env::remove_var(QUEUE_PRIORITY_ENV);
    }
}

/// FR-008 / AC-008.15 — rules.conf priority maps when env unset.
#[test]
#[serial]
fn fr008_spawn_request_rules_conf_priority() {
    unsafe {
        std::env::remove_var(QUEUE_PRIORITY_ENV);
    }
    let req = SpawnRequest::from_operator(
        vec!["cargo".into(), "build".into(), "--force".into()],
        std::env::current_dir().expect("cwd"),
        vec![],
        Some("high"),
    );
    assert_eq!(
        req.queue_priority,
        QueuePriority::High,
        "AC-008.15: rules.conf priority MUST populate SpawnRequest"
    );
}

/// FR-008 / AC-008.15 — Hypervisor nocache honors `SpawnRequest::new` operator env.
#[tokio::test]
#[serial]
async fn fr008_hypervisor_operator_env_critical_before_normal() {
    unsafe {
        std::env::set_var(QUEUE_PRIORITY_ENV, "critical");
    }

    let dir = TempDir::new().expect("tempdir");
    let order_path = dir.path().join("order.log");
    std::fs::write(&order_path, b"").expect("seed order log");

    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Arc::new(Hypervisor::with_thermal_gate(dir.path(), gate));

    let order_log = order_path.display().to_string();

    #[cfg(unix)]
    let hold_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "printf 'holder_start\n' >> '{}'; sleep 0.15; printf 'holder_end\n' >> '{}'",
            order_path.display(),
            order_path.display()
        ),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let hold_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!(
            "echo holder_start>>\"{}\" & timeout /T 2 /NOBREAK >NUL & echo holder_end>>\"{}\"",
            order_path.display(),
            order_path.display()
        ),
        "--force".to_string(),
    ];

    #[cfg(unix)]
    let critical_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("printf 'critical\n' >> '{order_log}'"),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let critical_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!("echo critical>>\"{order_log}\""),
        "--force".to_string(),
    ];

    #[cfg(unix)]
    let late_normal_argv = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("printf 'normal_late\n' >> '{order_log}'"),
        "--force".to_string(),
    ];
    #[cfg(windows)]
    let late_normal_argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        format!("echo normal_late>>\"{order_log}\""),
        "--force".to_string(),
    ];

    let cwd = dir.path().to_path_buf();
    let holder_hv = Arc::clone(&hv);
    let holder_cwd = cwd.clone();
    let holder = tokio::spawn(async move {
        holder_hv
            .run(
                SpawnRequest::new(hold_argv, holder_cwd, vec![])
                    .with_queue_priority(QueuePriority::Normal),
            )
            .await
            .expect("holder nocache run")
    });

    let holder_started_deadline = Instant::now() + Duration::from_secs(30);
    while !std::fs::read_to_string(&order_path).map(|s| s.contains("holder_start")).unwrap_or(false)
    {
        if Instant::now() >= holder_started_deadline {
            panic!(
                "holder_start never observed in {} within 30s; order file: {:?}",
                order_path.display(),
                std::fs::read_to_string(&order_path).unwrap_or_default()
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let critical_hv = Arc::clone(&hv);
    let critical_cwd = cwd.clone();
    let critical = tokio::spawn(async move {
        critical_hv
            .run(SpawnRequest::new(critical_argv, critical_cwd, vec![]))
            .await
            .expect("critical nocache run from operator env")
    });

    let late_normal_hv = Arc::clone(&hv);
    let late_normal = tokio::spawn(async move {
        late_normal_hv
            .run(
                SpawnRequest::new(late_normal_argv, cwd, vec![])
                    .with_queue_priority(QueuePriority::Normal),
            )
            .await
            .expect("late normal nocache run")
    });

    holder.await.expect("holder join");
    critical.await.expect("critical join");
    late_normal.await.expect("late normal join");

    let order = std::fs::read_to_string(&order_path).expect("read order log");
    let lines: Vec<&str> = order.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines,
        vec!["holder_start", "holder_end", "critical", "normal_late"],
        "AC-008.15: operator env on SpawnRequest::new MUST Critical-before-Normal"
    );

    assert_eq!(
        resolve_operator_queue_priority(Some("low")),
        QueuePriority::Critical,
        "resolver MUST still read env during test"
    );

    unsafe {
        std::env::remove_var(QUEUE_PRIORITY_ENV);
    }
}
