// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 × FR-007 — per-agent ResourceWatchSample for detected PIDs
//! FR: FR-006, FR-007
//!
//! AC-006.10 proc-scan agents carry live RSS (and FD on Linux) samples.

use sharecli_fleet::proc_scan::DetectedAgent;
use sharecli_fleet::proc_scan::{FakeProcSource, ProcSnapshot};
use sharecli_fleet::{
    watch_detected_agents, AgentResourceSample, DetectedAgentWatch, ResourceWatchSample,
};

fn fixture_agents() -> Vec<DetectedAgent> {
    vec![DetectedAgent {
        pid: std::process::id(),
        family: "sharecli-test",
        comm: "fr006-agent-pid-watch".into(),
    }]
}

/// FR-006 / AC-006.10 — live self-PID resource sample via AgentResourceSample.
#[test]
fn fr006_agent_resource_sample_self_pid() {
    let pid = std::process::id();
    let sample = AgentResourceSample::capture_for_pid(pid).expect("self PID MUST be sampleable");
    assert!(sample.mem_rss_bytes > 0, "RSS MUST be non-zero for live process");
    #[cfg(target_os = "linux")]
    assert!(sample.fd_count.unwrap_or(0) >= 3, "Linux self FD count MUST include stdio");
}

/// FR-006 / AC-006.10 — watch_detected_agents joins proc scan + per-PID watch.
#[test]
fn fr006_watch_detected_agents_self_row() {
    let rows = watch_detected_agents(&fixture_agents());
    assert_eq!(rows.len(), 1, "live self agent row MUST be watched");
    assert_eq!(rows[0].agent.family, "sharecli-test");
    assert!(rows[0].resource.mem_rss_bytes > 0);
}

/// FR-006 / AC-006.10 — dead PIDs are omitted rather than silent zero samples.
#[test]
fn fr006_watch_detected_agents_skips_dead_pid() {
    let src = FakeProcSource::new(vec![ProcSnapshot {
        pid: 999_999,
        ppid: 1,
        comm: "claude".into(),
        cmdline: vec!["claude".into()],
        state: 'R',
    }]);
    let agents = sharecli_fleet::scan_agents(&src);
    let rows = watch_detected_agents(&agents);
    assert!(rows.is_empty(), "dead PID MUST not produce a watch row");
}

/// FR-007 — Hypervisor self watch remains distinct from per-agent slice.
#[test]
fn fr007_host_resource_watch_still_host_scoped() {
    let host = ResourceWatchSample::capture().expect("host resource watch");
    assert!(host.load_1m >= 0.0);
    assert!(host.mem_rss_bytes > 0);
}

/// DetectedAgentWatch round-trip shape for TUI / ps --all consumers.
#[test]
fn fr006_detected_agent_watch_shape() {
    let row = DetectedAgentWatch {
        agent: fixture_agents()[0].clone(),
        resource: AgentResourceSample { mem_rss_bytes: 1_048_576, fd_count: Some(10) },
    };
    assert_eq!(row.agent.pid, std::process::id());
    assert_eq!(row.resource.mem_rss_bytes, 1_048_576);
}
