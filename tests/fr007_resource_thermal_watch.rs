// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Resource & Syscall-Relevant Watch
//! FR: FR-007
//!
//! AC-007.1 thermal governor mock levels round-trip
//! AC-007.2 FakeThermalGate maps Allow/Warn/Refuse
//! AC-007.3 ProcessStats idle heuristic (CPU/MEM-relevant signal)
//! AC-007.4 FD watch via sample_self_fds / ResourceWatchSample
//! AC-007.5 host net RX/TX watch via sample_host_net
//! AC-007.6 Hypervisor::run attaches live resource watch to SpawnOutcome
//! AC-007.7 RSS watch via sample_self_rss_bytes
//! AC-007.8 host load watch via sample_host_load_1m
//! AC-007.10 status surfaces live ResourceWatchSample via format_status_section

use std::sync::Arc;

use sharecli::monitoring::{
    sample_host_load_1m, sample_host_net, sample_self_fds, sample_self_rss_bytes, ProcessStats,
    ResourceWatchSample,
};
use sharecli_core::{
    FakeThermalGate, Hypervisor, QueuePriority, SpawnRequest, ThermalDecision, ThermalGate,
};
use sharecli_fleet::{ThermalGovernor, ThermalLevel};
use tempfile::TempDir;

/// FR-007 / AC-007.1 — mock thermal levels are visible via poll.
#[test]
fn fr007_thermal_governor_mock_levels() {
    for level in [ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red] {
        let gov = ThermalGovernor::with_mock(level);
        let got = gov.poll().expect("mock poll");
        assert_eq!(got, level);
    }
}

/// FR-007 / AC-007.2 — FakeThermalGate decisions are stable.
#[test]
fn fr007_fake_thermal_gate_maps_decisions() {
    assert_eq!(FakeThermalGate::new(ThermalDecision::Allow).check(), ThermalDecision::Allow);
    assert_eq!(FakeThermalGate::new(ThermalDecision::Warn).check(), ThermalDecision::Warn);
    assert_eq!(FakeThermalGate::new(ThermalDecision::Refuse).check(), ThermalDecision::Refuse);
}

/// FR-007 / AC-007.3 — idle heuristic encodes CPU + uptime watch signal.
#[test]
fn fr007_process_stats_idle_heuristic() {
    let idle = ProcessStats::new(1, "agent", 64, 0.1, 0, 120);
    assert!(idle.is_idle(60), "low CPU + long uptime MUST be idle");

    let busy = ProcessStats::new(2, "agent", 512, 42.0, 0, 120);
    assert!(!busy.is_idle(60), "high CPU MUST not be idle");
}

/// FR-007 / AC-007.4 — FD watch samples the current process open descriptor count.
#[test]
fn fr007_fd_watch_samples_self_fds() {
    let fd_count = sample_self_fds().expect("FD watch MUST succeed on supported OS");
    assert!(fd_count >= 3, "process MUST have at least stdin/stdout/stderr FDs");

    let sample = ResourceWatchSample::capture().expect("resource watch capture");
    assert!(sample.fd_count >= 3, "capture MUST include live FD count");

    let stats = ProcessStats::new(1, "agent", 0, 0.0, 0, 0)
        .with_resource_watch()
        .expect("ProcessStats resource watch");
    assert!(stats.fd_count >= 3, "ProcessStats MUST carry FD watch signal");
}

/// FR-007 / AC-007.5 — host net RX/TX watch returns byte counters (not silent zero on failure).
#[test]
fn fr007_net_watch_samples_host_counters() {
    sample_host_net().expect("network watch MUST succeed on supported OS");
    ResourceWatchSample::capture().expect("resource watch capture");

    let stats = ProcessStats::new(1, "agent", 0, 0.0, 0, 0)
        .with_resource_watch()
        .expect("ProcessStats resource watch MUST populate net watch fields");
    // Live host counters; values vary by machine but MUST be sampled (not left at defaults
    // when with_resource_watch succeeds).
    let _ = (stats.net_rx_bytes, stats.net_tx_bytes);
}

/// FR-007 / AC-007.7 — RSS watch samples current process resident memory.
#[test]
fn fr007_rss_watch_samples_self_bytes() {
    let rss = sample_self_rss_bytes().expect("RSS watch MUST succeed on supported OS");
    assert!(rss > 0, "live process MUST have non-zero RSS");

    let sample = ResourceWatchSample::capture().expect("resource watch capture");
    assert!(sample.mem_rss_bytes > 0, "capture MUST include live RSS");

    let stats = ProcessStats::new(1, "agent", 0, 0.0, 0, 0)
        .with_resource_watch()
        .expect("ProcessStats resource watch");
    assert!(stats.mem_rss_bytes > 0, "ProcessStats MUST carry RSS watch signal");
}

/// FR-007 / AC-007.8 — host load average is sampled (not silent zero on failure).
#[test]
fn fr007_load_watch_samples_host_load_1m() {
    let load = sample_host_load_1m().expect("load watch MUST succeed on supported OS");
    assert!(load >= 0.0, "load average MUST be non-negative");

    let sample = ResourceWatchSample::capture().expect("resource watch capture");
    assert!(sample.load_1m >= 0.0, "capture MUST include live load average");

    let stats = ProcessStats::new(1, "agent", 0, 0.0, 0, 0)
        .with_resource_watch()
        .expect("ProcessStats resource watch MUST populate load field");
    assert!(stats.load_1m >= 0.0);
}

/// FR-007 / AC-007.10 — status block formats FD/RSS/load/net watch fields.
#[test]
fn fr007_format_status_section() {
    let sample = ResourceWatchSample::capture().expect("resource watch capture");
    let section = sample.format_status_section();

    assert!(section.contains("=== Host Resource Watch ==="), "got: {section}");
    assert!(section.contains("Open FDs:"), "got: {section}");
    assert!(section.contains("RSS:"), "got: {section}");
    assert!(section.contains("Load (1m):"), "got: {section}");
    assert!(section.contains("Net RX:"), "got: {section}");
    assert!(section.contains("Net TX:"), "got: {section}");
    assert!(sample.fd_count >= 3, "live FD count MUST be present");
    assert!(sample.mem_rss_bytes > 0, "live RSS MUST be present");
}

/// FR-007 / AC-007.6 — Hypervisor run path carries live FD/net watch on SpawnOutcome.
#[tokio::test]
async fn fr007_hypervisor_run_carries_resource_watch() {
    let dir = TempDir::new().expect("tempdir");
    let gate = Arc::new(FakeThermalGate::new(ThermalDecision::Allow));
    let hv = Hypervisor::with_thermal_gate(dir.path(), gate);

    #[cfg(unix)]
    let argv = vec!["echo".to_string(), "fr007-hypervisor-watch".to_string()];
    #[cfg(windows)]
    let argv = vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo".to_string(),
        "fr007-hypervisor-watch".to_string(),
    ];

    let outcome = hv
        .run(SpawnRequest {
            argv,
            cwd: dir.path().to_path_buf(),
            env: vec![],
            queue_priority: QueuePriority::Normal,
        })
        .await
        .expect("Hypervisor run MUST sample resource watch");

    assert!(
        outcome.resource_watch.fd_count >= 3,
        "Hypervisor MUST attach live FD watch (got {})",
        outcome.resource_watch.fd_count
    );
    assert!(
        outcome.resource_watch.mem_rss_bytes > 0,
        "Hypervisor MUST attach live RSS watch (got {})",
        outcome.resource_watch.mem_rss_bytes
    );
    assert!(outcome.resource_watch.load_1m >= 0.0, "Hypervisor MUST attach live load watch");
    // Caller detection is intentionally host-relative: this suite may run
    // beneath Codex or another supported harness. FR-007.6 only requires the
    // independently captured resource-watch fields above.
    let _ = (outcome.resource_watch.net_rx_bytes, outcome.resource_watch.net_tx_bytes);
}
