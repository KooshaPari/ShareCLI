// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — thermal TUI pool + proc-scan operator panels (AC-007.71)
//! FR: FR-007
//!
//! Thermal TUI surfaces dedicated runtime pool + proc-scan status panels using the same
//! operator field shapes as tray AC-007.69 and dashboard AC-007.70.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};
use sharecli_thermal_tui::{pool_panel_lines, render, status_panel_lines, App, HELP_OVERLAY_HINT};

const SAMPLE_POOL: PoolOperatorPanel = PoolOperatorPanel {
    node_total: 2,
    node_idle: 1,
    bun_total: 1,
    bun_idle: 0,
    max_per_type: 4,
    healthy: true,
};

const SAMPLE_STATUS: StatusOperatorPanel =
    StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };

/// FR-007 / AC-007.71 — pool panel lines match tray operator formatter.
#[test]
fn fr007_thermal_tui_pool_panel_lines_tray_parity() {
    let lines = pool_panel_lines(Some(SAMPLE_POOL), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(
        text.contains("Node idle/total: 1/2"),
        "pool panel MUST show node capacity; got: {text}"
    );
    assert!(text.contains("healthy"), "pool panel MUST show health; got: {text}");

    let compact: String =
        pool_panel_lines(Some(SAMPLE_POOL), true).iter().map(|l| l.to_string()).collect();
    assert!(
        compact.contains("Pool node 2/1 idle · bun 1/0 idle · max 4 · healthy"),
        "compact pool MUST match tray line (AC-007.71); got: {compact}"
    );
}

/// FR-007 / AC-007.71 — status panel lines match tray proc-scan formatter.
#[test]
fn fr007_thermal_tui_status_panel_lines_tray_parity() {
    let lines = status_panel_lines(Some(SAMPLE_STATUS), false);
    let text: String = lines.iter().map(|l| l.to_string()).collect();
    assert!(text.contains("Scanned:  50"), "status panel MUST show scanned; got: {text}");
    assert!(text.contains("agent rows: 1"), "status panel MUST show agent rows; got: {text}");

    let compact: String =
        status_panel_lines(Some(SAMPLE_STATUS), true).iter().map(|l| l.to_string()).collect();
    assert!(
        compact.contains("Proc scan 50 · watched 1 · 2 managed · 1 agent row(s)"),
        "compact status MUST match tray line (AC-007.71); got: {compact}"
    );
}

/// FR-007 / AC-007.71 — headless thermal render includes pool + status operator panels.
#[test]
fn fr007_thermal_tui_render_includes_pool_status_panels() {
    let backend = TestBackend::new(120, 64);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = App::new(4).with_pool_status_panels(Some(SAMPLE_POOL), Some(SAMPLE_STATUS));
    app.update(ThermalLevel::Green, 1);

    terminal.draw(|f| render(f, &app)).expect("draw");
    let rendered: String =
        terminal.backend().buffer().content.iter().map(|c| c.symbol().to_string()).collect();

    assert!(
        rendered.contains("Runtime Pool") && rendered.contains("Node idle/total"),
        "thermal TUI MUST surface runtime pool panel (AC-007.71)"
    );
    assert!(
        rendered.contains("Proc Scan Status") && rendered.contains("Scanned:"),
        "thermal TUI MUST surface proc-scan status panel (AC-007.71)"
    );
    assert!(
        rendered.contains("Pool node 2/1 idle") || rendered.contains("Node idle/total: 1/2"),
        "thermal TUI MUST show pool capacity fields (AC-007.71)"
    );
}

/// FR-007 / AC-007.71 — keyboard help documents pool/status focus targets.
#[test]
fn fr007_thermal_tui_pool_status_keyboard_help() {
    assert!(HELP_OVERLAY_HINT.contains("2 pool"));
    assert!(HELP_OVERLAY_HINT.contains("3 status"));
    assert!(HELP_OVERLAY_HINT.contains("4 watch"));
    assert!(HELP_OVERLAY_HINT.contains("5 agents"));
}

/// FR-007 / AC-007.71 — `sharecli thermal` wires live pool/status poll hook.
#[test]
fn fr007_thermal_tui_main_wires_pool_status_poll() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("run_with_pool_status"),
        "thermal command MUST use run_with_pool_status (AC-007.71)"
    );
    assert!(
        main_rs.contains("build_pool_json"),
        "thermal poll MUST call build_pool_json (AC-007.71)"
    );
    assert!(
        main_rs.contains("build_status_json"),
        "thermal poll MUST call build_status_json (AC-007.71)"
    );
}
