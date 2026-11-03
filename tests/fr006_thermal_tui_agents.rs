// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-006 — thermal TUI DetectedAgent panel (proc scan + per-PID watch)
//! FR: FR-006, FR-007
//!
//! AC-006.9 `sharecli thermal` polls agent inventory each redraw.
//! AC-006.10 rows include per-PID RSS (and FD on Linux).
//! AC-006.40 flat Detected Agents lines show process state after PID (parity with proc text).

use std::collections::HashMap;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sharecli_fleet::proc_scan::DetectedAgent;
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fleet::CoalesceMeters;
use sharecli_fleet::SlotQueueMeters;
use sharecli_fleet::{AgentResourceSample, DetectedAgentWatch, ResourceWatchSample};
use sharecli_fuse::{NegDentryMeters, ReadCacheMeters, WriteSerializeMeters};
use sharecli_thermal_tui::{agent_lines, render, App};

const SAMPLE: ResourceWatchSample = ResourceWatchSample {
    fd_count: 12,
    net_rx_bytes: 0,
    net_tx_bytes: 0,
    mem_rss_bytes: 4096,
    load_1m: 0.5,
};

fn fixture_agents() -> Vec<DetectedAgentWatch> {
    vec![
        DetectedAgentWatch {
            agent: DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            resource: AgentResourceSample { mem_rss_bytes: 52_428_800, fd_count: Some(24) },
        },
        DetectedAgentWatch {
            agent: DetectedAgent { pid: 250, family: "cursor", comm: "cursor-agent".into() },
            resource: AgentResourceSample { mem_rss_bytes: 104_857_600, fd_count: None },
        },
    ]
}

fn fixture_state_by_pid() -> HashMap<u32, char> {
    let mut map = HashMap::new();
    map.insert(100, 'S');
    map.insert(250, 'R');
    map
}

/// FR-006 / AC-006.9 + AC-006.10 — agent_lines renders inventory + RSS rows.
#[test]
fn fr006_thermal_tui_agent_lines() {
    let lines = agent_lines(&fixture_agents(), &fixture_state_by_pid(), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Agents: 2"));
    assert!(text.contains("PID 100  S  claude"));
    assert!(text.contains("RSS 50M"));
    assert!(text.contains("PID 250  R  cursor"));
}

/// FR-006 / AC-006.40 — flat agent_lines show state letter after PID.
#[test]
fn fr006_thermal_tui_agent_lines_show_state() {
    let lines = agent_lines(&fixture_agents(), &fixture_state_by_pid(), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(
        text.contains("PID 100  S  claude") && text.contains("PID 250  R  cursor"),
        "flat lines MUST show state after PID; got: {text}"
    );
}

/// FR-006 / AC-006.40 — missing state shows `-` on flat lines.
#[test]
fn fr006_thermal_tui_agent_lines_missing_state_dash() {
    let lines = agent_lines(&fixture_agents(), &HashMap::new(), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(
        text.contains("PID 100  -  claude") && text.contains("PID 250  -  cursor"),
        "missing state MUST render `-`; got: {text}"
    );
}

/// FR-006 / AC-006.40 — compact flat summary includes state after PID.
#[test]
fn fr006_thermal_tui_agent_lines_compact_show_state() {
    let lines = agent_lines(&fixture_agents(), &fixture_state_by_pid(), true);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(
        text.contains("claude:100:S@") && text.contains("cursor:250:R@"),
        "compact flat MUST show state after PID; got: {text}"
    );
}

/// FR-006 / AC-006.9 — headless thermal render includes DetectedAgent panel.
#[test]
fn fr006_thermal_tui_render_includes_agent_panel() {
    // 64 rows: the full layout (title/thermal/gate/slots/pool/status/agents/watch/io/footer)
    // needs ~40 rows; the in-crate render tests use TestBackend::new(120, 64).
    let backend = TestBackend::new(120, 64);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = App::new(4)
        .with_operator_meters(
            Some(SAMPLE),
            ReadCacheMeters { hits: 1, misses: 0 },
            NegDentryMeters { hits: 0, misses: 0 },
            CoalesceMeters::default(),
            SlotQueueMeters::default(),
            WriteSerializeMeters::default(),
        )
        .with_detected_agents(fixture_agents())
        .with_agent_forest_state(fixture_state_by_pid());
    app.update(ThermalLevel::Green, 0);

    terminal.draw(|f| render(f, &app)).expect("draw");
    let rendered: String =
        terminal.backend().buffer().content.iter().map(|c| c.symbol().to_string()).collect();

    assert!(
        rendered.contains("Detected Agents") && rendered.contains("PID 100"),
        "thermal TUI MUST surface proc-scan agent inventory"
    );
    assert!(rendered.contains("RSS"), "thermal TUI MUST surface per-agent RSS watch");
    assert!(
        rendered.contains("PID 100  S  claude"),
        "flat thermal render MUST show process state after PID (AC-006.40); got excerpt: {}",
        &rendered.chars().take(500).collect::<String>()
    );
}
