// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal TUI gate panel parity with proc/status JSON gate
//! FR: FR-007, FR-011
//!
//! AC-007.26 thermal TUI gate derives ADMIT/DENY from gate_status_snapshot_with_rss

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fleet::{AgentResourceSample, DetectedAgent, DetectedAgentWatch};
use sharecli_thermal_tui::{gate_panel_lines, render, App};

const RSS_REFUSE_BYTES: u64 = 32 * 1_073_741_824;

/// FR-007 / AC-007.26 — RSS refuse tier surfaces DENY + RSS total + REFUSE contention.
#[test]
fn fr007_thermal_tui_gate_parity_rss_refuse() {
    let snap =
        sharecli_fleet::gate_status_snapshot_with_rss(ThermalLevel::Green, 1, RSS_REFUSE_BYTES);
    assert_eq!(snap.gate_decision, "DENY");
    assert_eq!(snap.agent_contention, "REFUSE");
    assert_eq!(snap.agent_total_rss_bytes, RSS_REFUSE_BYTES);

    let panel = gate_panel_lines(&snap, ThermalLevel::Green, false);
    let panel_text: String = panel.iter().map(|l| l.to_string()).collect();
    assert!(panel_text.contains("DENY"), "gate panel lines MUST include DENY; got: {panel_text}");
    assert!(
        panel_text.contains(&RSS_REFUSE_BYTES.to_string()),
        "gate panel lines MUST include agent RSS total; got: {panel_text}"
    );
    assert!(
        panel_text.contains("REFUSE"),
        "gate panel lines MUST include REFUSE contention; got: {panel_text}"
    );

    let backend = TestBackend::new(120, 64);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = App::new(4).with_detected_agents(vec![DetectedAgentWatch {
        agent: DetectedAgent { pid: 4242, family: "claude", comm: "claude".into() },
        resource: AgentResourceSample { mem_rss_bytes: RSS_REFUSE_BYTES, fd_count: Some(8) },
    }]);
    app.update(ThermalLevel::Green, 1);

    terminal.draw(|f| render(f, &app)).expect("draw");
    let rendered: String =
        terminal.backend().buffer().content.iter().map(|c| c.symbol().to_string()).collect();

    assert!(
        rendered.contains("DENY"),
        "thermal TUI render MUST show DENY for RSS refuse (AC-007.26); excerpt missing DENY"
    );
    assert!(
        rendered.contains(&RSS_REFUSE_BYTES.to_string()),
        "thermal TUI render MUST show agent RSS total (AC-007.26)"
    );
    assert!(
        rendered.contains("REFUSE"),
        "thermal TUI render MUST show REFUSE contention (AC-007.26)"
    );
}
