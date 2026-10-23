// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-004 — Process & Pool Health Status (status / HealthStatus / ProcessStats)
//! FR: FR-004
//!
//! Covers AC-004.1, AC-004.4, AC-004.5, AC-007.9 (FUSE read-coalesce in status),
//! AC-007.10 (host resource watch in status), AC-009.9 (FUSE neg dentry in status),
//! AC-008.11 (Hypervisor coalesce in status), AC-009.10 (FUSE write-serialize in status),
//! AC-010.11 (mesh Maildir depth in status),
//! AC-011.5 (thermal+agent gate in status).

use std::collections::HashMap;

use sharecli::monitoring::{HealthStatus, ProcessStats, ResourceWatchSample};
use sharecli::runtime::{ProcState, ProcessInfo, ProcessPool, SharedRuntime};
use sharecli_fleet::format_gate_status_section;
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use sharecli_fuse::{
    global_neg_dentry_meters, global_read_cache_meters, global_write_serialize_meters,
};
use sharecli_ipc::{global_coalesce_meters, global_slot_queue_meters};
use sharecli_mesh::{capture_maildir_status, MESH_QUEUE_ENV};

/// Mirror of the per-harness aggregation + status tables in `commands::status`.
fn format_status(
    processes: &[ProcessInfo],
    pool_status: &sharecli::runtime::PoolStatus,
    used_mb: u64,
    total_mb: u64,
) -> String {
    let mut by_harness: HashMap<&str, (usize, u64)> = HashMap::new();
    for proc in processes {
        let h = proc.harness.as_deref().unwrap_or("unknown");
        let entry = by_harness.entry(h).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += proc.memory_mb;
    }

    let mut out = String::from("=== Process Status ===\n\n");
    out.push_str(&format!("Total: {} processes\n\n", processes.len()));
    out.push_str(&format!("{:<15} {:<10} {:<15}\n", "HARNESS", "COUNT", "MEMORY(MB)"));
    out.push_str(&format!("{}\n", "-".repeat(40)));
    for (h, (count, mem)) in &by_harness {
        out.push_str(&format!("{h:<15} {count:<10} {mem:<15}\n"));
    }

    out.push_str("\n=== Shared Runtime Pool ===\n\n");
    out.push_str(&format!("{:<10} {:<10} {:<10}\n", "TYPE", "TOTAL", "IDLE"));
    out.push_str(&format!("{}\n", "-".repeat(30)));
    out.push_str(&format!(
        "{:<10} {:<10} {:<10}\n",
        "node", pool_status.node_total, pool_status.node_idle
    ));
    out.push_str(&format!(
        "{:<10} {:<10} {:<10}\n",
        "bun", pool_status.bun_total, pool_status.bun_idle
    ));
    out.push_str(&format!("\nMax per type: {}\n", pool_status.max_per_type));

    let pct = used_mb.saturating_mul(100).checked_div(total_mb).unwrap_or(0);
    out.push_str("\n=== System Memory ===\n\n");
    out.push_str(&format!("Used: {used_mb} MB / {total_mb} MB ({pct}%)\n"));
    let thermal =
        ThermalGovernor::with_mock(ThermalLevel::Green).poll().expect("mock thermal poll");
    out.push_str(&format_gate_status_section(thermal, 0));
    let resource_watch = ResourceWatchSample::capture().expect("resource watch capture");
    out.push_str(&resource_watch.format_status_section());
    out.push_str(&global_read_cache_meters().format_status_section());
    out.push_str(&global_neg_dentry_meters().format_status_section());
    out.push_str(&global_coalesce_meters().format_status_section());
    out.push_str(&global_slot_queue_meters().format_status_section());
    if let Ok(Some(st)) = capture_maildir_status() {
        out.push_str(&st.format_status_section());
    }
    out.push_str(&global_write_serialize_meters().format_status_section());
    out
}

fn sample_process(pid: u32, name: &str, memory_mb: u64, harness: &str) -> ProcessInfo {
    ProcessInfo {
        pid,
        name: name.to_string(),
        cmd: vec![name.to_string()],
        memory_mb,
        start_time: 0,
        cpu_percent: 0.0,
        project: Some("demo".to_string()),
        harness: Some(harness.to_string()),
        ppid: None,
        cwd: None,
        env_count: 0,
        state: ProcState::default(),
        disk_read_bytes: None,
        disk_write_bytes: None,
        fd_count: None,
        thread_count: None,
    }
}

/// FR-004 / AC-004.1 — `status` prints per-harness table, pool table, system memory.
#[tokio::test]
async fn fr004_status_prints_harness_table() {
    let processes = vec![
        sample_process(101, "claude", 64, "claude"),
        sample_process(102, "claude", 32, "claude"),
        sample_process(103, "codex", 128, "codex"),
    ];

    let runtime = SharedRuntime::new(4);
    let pool_status = runtime.status().await;
    let pool = ProcessPool::new();
    let (used_mb, total_mb) = pool.system_memory_usage().await;

    let out = format_status(&processes, &pool_status, used_mb, total_mb);

    assert!(out.contains("=== Process Status ==="), "got: {out}");
    assert!(
        out.contains("HARNESS") && out.contains("COUNT") && out.contains("MEMORY(MB)"),
        "got: {out}"
    );
    assert!(
        out.contains("claude") && out.contains("codex"),
        "harness rows MUST appear; got: {out}"
    );
    assert!(
        out.contains("=== Shared Runtime Pool ==="),
        "status MUST include shared-runtime pool table; got: {out}"
    );
    assert!(out.contains("TYPE") && out.contains("TOTAL") && out.contains("IDLE"), "got: {out}");
    assert!(
        out.contains("node") && out.contains("bun"),
        "pool rows MUST list node and bun; got: {out}"
    );
    assert!(out.contains("Max per type:"), "got: {out}");
    assert!(
        out.contains("=== System Memory ===") && out.contains("Used:"),
        "status MUST include system-memory line; got: {out}"
    );
    assert!(total_mb > 0, "system_memory_usage MUST report a non-zero total");
    assert!(
        out.contains("=== Host Resource Watch ===")
            && out.contains("Open FDs:")
            && out.contains("RSS:")
            && out.contains("Load (1m):")
            && out.contains("Net RX:")
            && out.contains("Net TX:"),
        "status MUST surface host resource watch (AC-007.10); got: {out}"
    );
    assert!(
        out.contains("=== FUSE Read Coalesce ===")
            && out.contains("Cache hits:")
            && out.contains("Cache misses:")
            && out.contains("Hit rate:"),
        "status MUST surface FUSE read-coalesce meters (AC-007.9); got: {out}"
    );
    assert!(
        out.contains("=== FUSE Negative Dentry ===")
            && out.contains("Neg hits:")
            && out.contains("Neg misses:"),
        "status MUST surface FUSE negative-dentry meters (AC-009.9); got: {out}"
    );
    assert!(
        out.contains("=== Hypervisor Coalesce ===")
            && out.contains("Cache hits:")
            && out.contains("Cache misses:")
            && out.contains("Nocache runs:")
            && out.contains("Hit rate:"),
        "status MUST surface Hypervisor coalesce meters (AC-008.11); got: {out}"
    );
    assert!(
        out.contains("=== Hypervisor SlotQueue ===")
            && out.contains("Acquires:")
            && out.contains("Waits:")
            && out.contains("Timeouts:"),
        "status MUST surface Hypervisor SlotQueue meters (AC-008.12); got: {out}"
    );

    // AC-010.11 — when mesh queue is configured, status surfaces Maildir depth.
    let mesh_dir = tempfile::tempdir().expect("mesh tempdir");
    {
        use serde_json::json;
        use sharecli_mesh::MaildirQueue;
        let q = MaildirQueue::open(mesh_dir.path()).expect("open mesh queue");
        q.enqueue(json!({"probe": true}), 0).expect("enqueue");
    }
    unsafe {
        std::env::set_var(MESH_QUEUE_ENV, mesh_dir.path());
    }
    let mesh_out = format_status(&processes, &pool_status, used_mb, total_mb);
    assert!(
        mesh_out.contains("=== Mesh Maildir Queue ===")
            && mesh_out.contains("Ready:")
            && mesh_out.contains("In-flight:")
            && mesh_out.contains("Pending:"),
        "status MUST surface mesh Maildir depth when queue exists (AC-010.11); got: {mesh_out}"
    );
    unsafe {
        std::env::remove_var(MESH_QUEUE_ENV);
    }

    assert!(
        out.contains("=== FUSE Write Serialize ===")
            && out.contains("Passthrough:")
            && out.contains("Stages:")
            && out.contains("Commits:")
            && out.contains("Discards:"),
        "status MUST surface FUSE write-serialize meters (AC-009.10); got: {out}"
    );
    assert!(
        out.contains("=== Thermal Gate (FR-011) ===")
            && out.contains("Thermal level: GREEN")
            && out.contains("Gate decision: [ADMIT]"),
        "status MUST surface thermal+agent gate section (AC-011.5); got: {out}"
    );
}

/// FR-004 / AC-004.4 — `HealthStatus::mark_unhealthy` increments `checks_failed`.
#[test]
fn fr004_health_status_marks_unhealthy() {
    let mut status = HealthStatus::new();
    assert!(status.healthy);
    assert_eq!(status.checks_failed, 0);
    let passed_before = status.checks_passed;

    status.mark_unhealthy("probe timeout");

    assert!(!status.healthy, "mark_unhealthy MUST clear healthy");
    assert_eq!(status.checks_failed, 1, "mark_unhealthy MUST increment checks_failed");
    assert_eq!(status.checks_passed, passed_before, "mark_unhealthy MUST NOT bump checks_passed");

    status.mark_unhealthy("second failure");
    assert_eq!(status.checks_failed, 2);
    assert!(!status.healthy);
}

/// FR-004 / AC-004.5 — `ProcessStats::is_idle` needs uptime > threshold AND cpu < 1.0.
#[test]
fn fr004_process_stats_idle_threshold() {
    let threshold = 60u64;

    let idle = ProcessStats {
        pid: 1,
        name: "worker".to_string(),
        memory_mb: 32,
        cpu_percent: 0.5,
        start_time: 1_000,
        uptime_seconds: threshold + 1,
        fd_count: 0,
        net_rx_bytes: 0,
        net_tx_bytes: 0,
        mem_rss_bytes: 0,
        load_1m: 0.0,
    };
    assert!(idle.is_idle(threshold), "uptime>threshold and cpu<1.0 MUST be idle");

    let busy_cpu = ProcessStats {
        pid: 2,
        name: "worker".to_string(),
        memory_mb: 32,
        cpu_percent: 1.0,
        start_time: 1_000,
        uptime_seconds: threshold + 100,
        fd_count: 0,
        net_rx_bytes: 0,
        net_tx_bytes: 0,
        mem_rss_bytes: 0,
        load_1m: 0.0,
    };
    assert!(!busy_cpu.is_idle(threshold), "cpu_percent >= 1.0 MUST NOT be idle");

    let too_young = ProcessStats {
        pid: 3,
        name: "worker".to_string(),
        memory_mb: 32,
        cpu_percent: 0.0,
        start_time: 1_000,
        uptime_seconds: threshold,
        fd_count: 0,
        net_rx_bytes: 0,
        net_tx_bytes: 0,
        mem_rss_bytes: 0,
        load_1m: 0.0,
    };
    assert!(!too_young.is_idle(threshold), "uptime_seconds <= threshold MUST NOT be idle");

    let boundary_ok = ProcessStats {
        pid: 4,
        name: "worker".to_string(),
        memory_mb: 8,
        cpu_percent: 0.99,
        start_time: 1_000,
        uptime_seconds: threshold + 1,
        fd_count: 0,
        net_rx_bytes: 0,
        net_tx_bytes: 0,
        mem_rss_bytes: 0,
        load_1m: 0.0,
    };
    assert!(boundary_ok.is_idle(threshold));
}
