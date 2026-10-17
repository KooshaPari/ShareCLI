// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal TUI operator watch panels (Feb harness dashboard slice)
//! FR: FR-007
//!
//! AC-007.9 FUSE read-coalesce meters in `sharecli thermal` TUI
//! AC-007.10 host resource watch in `sharecli thermal` TUI
//! AC-007.12 thermal TUI host resource watch net RX/TX parity with status
//! AC-008.11 Hypervisor coalesce meters in `sharecli thermal` TUI
//! AC-006.9 host agent inventory in `sharecli thermal` TUI (FR-006 proc scan)

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fleet::CoalesceMeters;
use sharecli_fleet::SlotQueueMeters;
use sharecli_fleet::{AgentResourceSample, DetectedAgent, DetectedAgentWatch, ResourceWatchSample};
use sharecli_fuse::{NegDentryMeters, ReadCacheMeters, WriteSerializeMeters};
use sharecli_mesh::MaildirStatus;
use sharecli_thermal_tui::{
    fuse_coalesce_lines, fuse_neg_dentry_lines, fuse_write_serialize_lines, host_agent_lines,
    hypervisor_coalesce_lines, hypervisor_slot_queue_lines, mesh_maildir_lines, render,
    resource_watch_lines, App,
};

const SAMPLE: ResourceWatchSample = ResourceWatchSample {
    fd_count: 24,
    net_rx_bytes: 4096,
    net_tx_bytes: 2048,
    mem_rss_bytes: 2_097_152,
    load_1m: 0.75,
};

const FUSE_METERS: ReadCacheMeters = ReadCacheMeters { hits: 5, misses: 2 };
const NEG_METERS: NegDentryMeters = NegDentryMeters { hits: 3, misses: 1 };
const COALESCE_METERS: CoalesceMeters = CoalesceMeters { hits: 9, misses: 3, nocache_runs: 2 };
const WRITE_SERIALIZE_METERS: WriteSerializeMeters =
    WriteSerializeMeters { passthrough_writes: 4, stages: 2, commits: 1, discards: 1 };
const SLOT_QUEUE_METERS: SlotQueueMeters = SlotQueueMeters { acquires: 5, waits: 3, timeouts: 1 };

fn sample_mesh_maildir() -> MaildirStatus {
    MaildirStatus {
        path: std::path::PathBuf::from("/tmp/sharecli-mesh-queue"),
        ready: 2,
        in_flight: 1,
        pending: 3,
    }
}

/// FR-007 / AC-007.10 — thermal TUI renders host resource watch lines.
#[test]
fn fr007_thermal_tui_resource_watch_lines() {
    let lines = resource_watch_lines(Some(SAMPLE), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Open FDs:") && text.contains("24"));
    assert!(text.contains("RSS:") && text.contains("2097152"));
    assert!(text.contains("Load (1m):") && text.contains("0.75"));
}

/// FR-007 / AC-007.12 — thermal TUI host resource watch surfaces net RX/TX byte counters.
#[test]
fn fr007_thermal_tui_resource_watch_net_lines() {
    let lines = resource_watch_lines(Some(SAMPLE), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(
        text.contains("Net RX:") && text.contains("4096"),
        "thermal TUI watch MUST surface net RX bytes (AC-007.12); got: {text}"
    );
    assert!(
        text.contains("Net TX:") && text.contains("2048"),
        "thermal TUI watch MUST surface net TX bytes (AC-007.12); got: {text}"
    );

    let compact = resource_watch_lines(Some(SAMPLE), true);
    let compact_text: String = compact.iter().map(|l| l.to_string()).collect();
    assert!(
        compact_text.contains("RX:4096") && compact_text.contains("TX:2048"),
        "compact thermal watch MUST include net counters (AC-007.12); got: {compact_text}"
    );
}

/// FR-007 / AC-007.9 — thermal TUI renders FUSE read-coalesce meters.
#[test]
fn fr007_thermal_tui_fuse_coalesce_lines() {
    let lines = fuse_coalesce_lines(FUSE_METERS, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Cache hits:") && text.contains('5'));
    assert!(text.contains("Cache misses:") && text.contains('2'));
    assert!(text.contains("Hit rate:") && text.contains("71"));
}

/// FR-009 / AC-009.9 — thermal TUI renders FUSE negative-dentry meters.
#[test]
fn fr009_thermal_tui_neg_dentry_lines() {
    let lines = fuse_neg_dentry_lines(NEG_METERS, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Neg hits:") && text.contains('3'));
    assert!(text.contains("Neg misses:") && text.contains('1'));
    assert!(text.contains("Hit rate:") && text.contains("75"));
}

/// FR-008 / AC-008.11 — thermal TUI renders Hypervisor coalesce cache meters.
#[test]
fn fr008_thermal_tui_hypervisor_coalesce_lines() {
    let lines = hypervisor_coalesce_lines(COALESCE_METERS, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Coalesce hits:") && text.contains('9'));
    assert!(text.contains("Coalesce misses:") && text.contains('3'));
    assert!(text.contains("Nocache runs:") && text.contains('2'));
    assert!(text.contains("Hit rate:") && text.contains("75"));
}

/// FR-008 / AC-008.12 — thermal TUI renders Hypervisor SlotQueue meters.
#[test]
fn fr008_thermal_tui_slot_queue_lines() {
    let lines = hypervisor_slot_queue_lines(SLOT_QUEUE_METERS, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Slot acquires:") && text.contains('5'));
    assert!(text.contains("Slot waits:") && text.contains('3'));
    assert!(text.contains("Slot timeouts:") && text.contains('1'));
}

/// FR-010 / AC-010.11 — thermal TUI renders mesh Maildir queue depth.
#[test]
fn fr010_thermal_tui_mesh_maildir_lines() {
    let lines = mesh_maildir_lines(Some(sample_mesh_maildir()), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Mesh ready:") && text.contains('2'));
    assert!(text.contains("Mesh in-flight:") && text.contains('1'));
    assert!(text.contains("Mesh pending:") && text.contains('3'));
}

/// FR-009 / AC-009.10 — thermal TUI renders FUSE write-serialize meters.
#[test]
fn fr009_thermal_tui_write_serialize_lines() {
    let lines = fuse_write_serialize_lines(WRITE_SERIALIZE_METERS, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Passthrough:") && text.contains('4'));
    assert!(text.contains("Stages:") && text.contains('2'));
    assert!(text.contains("Commits:") && text.contains('1'));
    assert!(text.contains("Discards:") && text.contains('1'));
}

/// FR-006 / AC-006.9 — thermal TUI renders host agent inventory lines.
#[test]
fn fr006_thermal_tui_host_agent_lines() {
    let agents = vec![DetectedAgent { pid: 4242, family: "claude", comm: "claude".into() }];
    let lines = host_agent_lines(&agents, false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Host agents:") && text.contains("claude"));
    assert!(text.contains("4242"));

    let empty = host_agent_lines(&[], false);
    let empty_text: String = empty.iter().map(|l| l.to_string()).collect();
    assert!(empty_text.contains("none detected"));
}

/// FR-007 / AC-007.9 + AC-007.10 + FR-009 AC-009.9 — headless thermal render includes panels.
#[test]
fn fr007_thermal_tui_render_includes_operator_panels() {
    let backend = TestBackend::new(120, 64);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = App::new(4)
        .with_operator_meters(
            Some(SAMPLE),
            FUSE_METERS,
            NEG_METERS,
            COALESCE_METERS,
            SLOT_QUEUE_METERS,
            WRITE_SERIALIZE_METERS,
        )
        .with_maildir_status(Some(sample_mesh_maildir()))
        .with_detected_agents(vec![DetectedAgentWatch {
            agent: DetectedAgent { pid: 100, family: "forge", comm: "forge".into() },
            resource: AgentResourceSample { mem_rss_bytes: 8_388_608, fd_count: Some(16) },
        }]);
    app.update(ThermalLevel::Green, 1);

    terminal.draw(|f| render(f, &app)).expect("draw");
    let rendered: String =
        terminal.backend().buffer().content.iter().map(|c| c.symbol().to_string()).collect();

    assert!(
        rendered.contains("Host Resource Watch") && rendered.contains("Open FDs:"),
        "thermal TUI MUST surface host resource watch; got excerpt missing panels"
    );
    assert!(
        rendered.contains("Net RX:") && rendered.contains("Net TX:"),
        "thermal TUI MUST surface host net RX/TX counters (AC-007.12)"
    );
    assert!(
        rendered.contains("Detected Agents") && rendered.contains("forge"),
        "thermal TUI MUST surface host agent inventory"
    );
    assert!(
        rendered.contains("Hypervisor IO Meters") && rendered.contains("Coalesce hits:"),
        "thermal TUI MUST surface Hypervisor coalesce meters"
    );
    assert!(
        rendered.contains("Slot acquires:"),
        "thermal TUI MUST surface Hypervisor SlotQueue meters (AC-008.12)"
    );
    assert!(
        rendered.contains("Mesh ready:") && rendered.contains("Mesh pending:"),
        "thermal TUI MUST surface mesh Maildir depth (AC-010.11)"
    );
    assert!(rendered.contains("Cache hits:"), "thermal TUI MUST surface FUSE read-coalesce meters");
    assert!(
        rendered.contains("Neg hits:") && rendered.contains("Neg misses:"),
        "thermal TUI MUST surface FUSE negative-dentry meters"
    );
    assert!(
        rendered.contains("Passthrough:"),
        "thermal TUI MUST surface FUSE write-serialize meters (AC-009.10)"
    );
}
