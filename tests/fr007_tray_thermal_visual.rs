// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — tray thermal gate visual indicator parity (AC-007.57)
//! FR: FR-007
//!
//! Linux, Swift, and Windows tray icon/badge/color MUST derive from
//! `gate.thermal_pressure` + `gate_decision` using shared severity→visual helpers
//! (parity with dashboard thermal/gate styling).

use sharecli_tray_linux::ipc::GateStatusSnapshot;
use sharecli_tray_linux::operator_display as linux_display;
use sharecli_tray_linux::operator_display::TrayGateSeverity;
use sharecli_tray_windows::operator_display as win_display;

/// FR-007 / AC-007.57 — golden severity matrix for ADMIT/GREEN (normal).
#[test]
fn fr007_tray_thermal_visual_admit_green_normal() {
    let v = linux_display::resolve_tray_gate_visual("GREEN", "ADMIT", true);
    assert_eq!(v.severity, TrayGateSeverity::Normal);
    assert_eq!(v.decision_class, "gate-admit");
    assert_eq!(v.thermal_class, "");
    assert_eq!(v.color_hex, "#3fb950");
    assert_eq!(v.badge_label, "Normal");
    assert_eq!(v.linux_icon_name, "utilities-system-monitor");
    assert_eq!(v.swift_symbol_name, "cpu");
}

/// FR-007 / AC-007.57 — YELLOW thermal elevates to warning even when ADMIT.
#[test]
fn fr007_tray_thermal_visual_yellow_warning() {
    let v = linux_display::resolve_tray_gate_visual("YELLOW", "ADMIT", true);
    assert_eq!(v.severity, TrayGateSeverity::Warning);
    assert_eq!(v.decision_class, "gate-admit");
    assert_eq!(v.thermal_class, "warning");
    assert_eq!(v.badge_label, "Warning");
    assert_eq!(v.linux_icon_name, "dialog-warning");
}

/// FR-007 / AC-007.57 — DENY + RED maps to critical (dashboard gate-deny + critical thermal).
#[test]
fn fr007_tray_thermal_visual_deny_red_critical() {
    let v = linux_display::resolve_tray_gate_visual("RED", "DENY", true);
    assert_eq!(v.severity, TrayGateSeverity::Critical);
    assert_eq!(v.decision_class, "gate-deny");
    assert_eq!(v.thermal_class, "critical");
    assert_eq!(v.color_hex, "#f85149");
    assert_eq!(v.badge_label, "Critical");
    assert_eq!(v.linux_icon_name, "dialog-error");
    assert_eq!(v.swift_symbol_name, "flame.fill");
}

/// FR-007 / AC-007.57 — THROTTLE decision maps to gate-unavailable styling.
#[test]
fn fr007_tray_thermal_visual_throttle_unavailable_class() {
    let v = linux_display::resolve_tray_gate_visual("GREEN", "THROTTLE", true);
    assert_eq!(v.severity, TrayGateSeverity::Warning);
    assert_eq!(v.decision_class, "gate-unavailable");
    assert_eq!(v.badge_label, "Warning");
}

/// FR-007 / AC-007.57 — disconnected IPC → offline visual tokens.
#[test]
fn fr007_tray_thermal_visual_offline_when_disconnected() {
    let v = linux_display::resolve_tray_gate_visual("GREEN", "ADMIT", false);
    assert_eq!(v.severity, TrayGateSeverity::Offline);
    assert_eq!(v.badge_label, "Offline");
    assert_eq!(v.linux_icon_name, "network-offline");
}

/// FR-007 / AC-007.57 — Windows Rust helpers stay token-identical to Linux.
#[test]
fn fr007_tray_thermal_visual_windows_linux_parity() {
    let cases = [
        ("GREEN", "ADMIT", true),
        ("YELLOW", "ADMIT", true),
        ("RED", "DENY", true),
        ("GREEN", "THROTTLE", true),
        ("UNAVAILABLE", "UNAVAILABLE", false),
    ];
    for (thermal, decision, connected) in cases {
        let lv = linux_display::resolve_tray_gate_visual(thermal, decision, connected);
        let wv = win_display::resolve_tray_gate_visual(thermal, decision, connected);
        assert_eq!(
            format!("{:?}", lv.severity),
            format!("{:?}", wv.severity),
            "{thermal}/{decision}"
        );
        assert_eq!(lv.decision_class, wv.decision_class);
        assert_eq!(lv.thermal_class, wv.thermal_class);
        assert_eq!(lv.color_hex, wv.color_hex);
        assert_eq!(lv.badge_label, wv.badge_label);
        assert_eq!(lv.linux_icon_name, wv.linux_icon_name);
        assert_eq!(lv.swift_symbol_name, wv.swift_symbol_name);
    }
}

/// FR-007 / AC-007.57 — gate snapshot helper used by tray refresh paths.
#[test]
fn fr007_tray_thermal_visual_from_gate_snapshot() {
    let gate = GateStatusSnapshot {
        thermal_pressure: "YELLOW".into(),
        detected_agents: 1,
        agent_total_rss_bytes: 512,
        agent_contention: "WARN".into(),
        gate_decision: "THROTTLE".into(),
    };
    let v = linux_display::resolve_tray_gate_visual_from_gate(&gate, true);
    assert_eq!(v.severity, TrayGateSeverity::Warning);
    assert_eq!(v.decision_class, "gate-unavailable");
    assert_eq!(v.thermal_class, "warning");
}

/// FR-007 / AC-007.57 — Linux tray wires icon + NeedsAttention from gate visual.
#[test]
fn fr007_tray_thermal_visual_linux_main_wires_icon() {
    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("gate_visual.linux_icon_name"),
        "Linux tray icon MUST follow resolve_tray_gate_visual (AC-007.57)"
    );
    assert!(
        main_rs.contains("fn status(&self) -> ksni::Status"),
        "Linux tray MUST expose SNI attention for warn/critical (AC-007.57)"
    );
    assert!(
        main_rs.contains("resolve_tray_gate_visual_from_gate"),
        "Linux refresh MUST derive gate visual from monitoring.report gate (AC-007.57)"
    );
}

/// FR-007 / AC-007.57 — Swift OperatorDisplay + menu bar icon wiring.
#[test]
fn fr007_tray_thermal_visual_swift_wires_operator_display() {
    let op = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift");
    assert!(
        op.contains("resolveTrayGateVisual"),
        "Swift OperatorDisplay MUST expose visual resolver (AC-007.57)"
    );
    assert!(
        op.contains("gate-admit"),
        "Swift visual resolver MUST map gate decision CSS classes (AC-007.57)"
    );

    let app = include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/AppEntry.swift");
    assert!(
        app.contains("visual.swiftSymbolName"),
        "macOS status item MUST update icon from gate visual (AC-007.57)"
    );

    let popover =
        include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift");
    assert!(
        popover.contains("thermalBadge"),
        "Tray popover MUST show thermal badge from gate visual (AC-007.57)"
    );

    let state = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift");
    assert!(
        state.contains("sharecliHealthChanged"),
        "AppState refresh MUST notify menu bar for visual updates (AC-007.57)"
    );
}

/// FR-007 / AC-007.57 — Windows WinUI thermal badge + colored gate row.
#[test]
fn fr007_tray_thermal_visual_windows_wires_operator_display() {
    let op = include_str!("../windows/ShareCLITray/OperatorDisplay.cs");
    assert!(
        op.contains("ResolveTrayGateVisual"),
        "C# OperatorDisplay MUST expose visual resolver (AC-007.57)"
    );
    assert!(
        op.contains("SeverityBrush"),
        "C# OperatorDisplay MUST map severity → brush (AC-007.57)"
    );

    let xaml = include_str!("../windows/ShareCLITray/TrayWindow.xaml");
    assert!(
        xaml.contains("ThermalBadgeText"),
        "TrayWindow XAML MUST include thermal badge row (AC-007.57)"
    );

    let code = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        code.contains("ResolveTrayGateVisual"),
        "TrayWindow MUST bind thermal badge from gate visual (AC-007.57)"
    );
    assert!(
        code.contains("SeverityBrush"),
        "TrayWindow MUST color gate row by severity (AC-007.57)"
    );
}

/// FR-007 / AC-007.58 — Swift HealthView metric cards + thermal gate detail use gate visual.
#[test]
fn fr007_tray_thermal_visual_swift_health_view_wires_gate_visual() {
    // The Health view moved from DashboardView.swift to HealthPage.swift
    // (ThermalGateSubpage renders the badge chip via gateVisual.badgeLabel).
    let health = include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/HealthPage.swift");
    assert!(
        health.contains("resolveTrayGateVisual"),
        "HealthView MUST resolve tray gate visual (AC-007.58)"
    );
    assert!(
        health.contains("gateVisual"),
        "HealthView MUST expose gateVisual computed property (AC-007.58)"
    );
    assert!(
        health.contains("gateVisual.badgeLabel"),
        "HealthView MUST show thermal gate badge chip (AC-007.58)"
    );
    assert!(
        health.contains("gateVisual.swiftColor"),
        "HealthView metric cards MUST use severity colors (AC-007.58)"
    );
    assert!(
        health.contains("gateVisual.badgeLabel"),
        "HealthView Thermal Gate card MUST show badge label (AC-007.58)"
    );
    assert!(
        !health.contains("h.healthy ? \"Healthy\""),
        "HealthView MUST NOT use generic healthy/warning Status card (AC-007.58)"
    );
}

/// FR-007 / AC-007.59 — Swift TrayPopoverView stats row Status cell uses gate visual.
#[test]
fn fr007_tray_thermal_visual_swift_popover_stats_row_wires_gate_visual() {
    let popover =
        include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift");
    assert!(
        popover.contains("gateVisual.swiftSymbolName"),
        "Tray popover stats row MUST use gate visual icon (AC-007.59)"
    );
    assert!(
        popover.contains("gateVisual.badgeLabel"),
        "Tray popover stats row MUST use gate visual badge label (AC-007.59)"
    );
    assert!(
        popover.contains("gateVisual.swiftColor"),
        "Tray popover stats row MUST use gate visual severity color (AC-007.59)"
    );
    assert!(
        !popover.contains("state.health?.healthy == true ? \"Healthy\""),
        "Tray popover stats row MUST NOT use generic healthy/warning Status cell (AC-007.59)"
    );
}

/// FR-007 / AC-007.61 — Linux SNI tooltip summary uses gate visual badge_label.
#[test]
fn fr007_tray_thermal_visual_linux_tooltip_wires_gate_visual() {
    let op = include_str!("../crates/sharecli-tray-linux/src/operator_display.rs");
    assert!(
        op.contains("format_tray_tooltip_summary_line"),
        "Linux OperatorDisplay MUST expose tooltip summary formatter (AC-007.61)"
    );
    assert!(
        op.contains("format_tray_tooltip_offline_line"),
        "Linux OperatorDisplay MUST expose offline tooltip formatter (AC-007.61)"
    );

    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("format_tray_tooltip_summary_line"),
        "Linux tray tool_tip MUST bind summary from gate visual (AC-007.61)"
    );
    assert!(
        main_rs.contains("gate_visual"),
        "Linux tray tool_tip MUST use gate_visual for severity (AC-007.61)"
    );
    assert!(
        !main_rs.contains("UNHEALTHY"),
        "Linux tray tool_tip MUST NOT use generic UNHEALTHY suffix (AC-007.61)"
    );
    assert!(
        !main_rs.contains("h.healthy"),
        "Linux tray tool_tip MUST NOT derive severity from health.healthy (AC-007.61)"
    );
}

/// FR-007 / AC-007.63 — Linux SNI menu header leads with gate visual badge_label.
#[test]
fn fr007_tray_thermal_visual_linux_menu_header_wires_gate_visual() {
    let op = include_str!("../crates/sharecli-tray-linux/src/operator_display.rs");
    assert!(
        op.contains("format_tray_menu_header_line"),
        "Linux OperatorDisplay MUST expose menu header formatter (AC-007.63)"
    );
    assert!(
        op.contains("format_tray_menu_header_offline_line"),
        "Linux OperatorDisplay MUST expose offline menu header formatter (AC-007.63)"
    );

    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("format_tray_menu_header_line"),
        "Linux tray menu MUST bind header from gate visual (AC-007.63)"
    );
    assert!(
        main_rs.contains("format_tray_menu_header_offline_line"),
        "Linux tray menu MUST bind offline header from gate visual (AC-007.63)"
    );
    assert!(
        !main_rs.contains("\"Daemon offline\""),
        "Linux tray menu MUST NOT use bare Daemon offline header (AC-007.63)"
    );
    assert!(
        !main_rs.contains("process(es) · {} / {} MB\""),
        "Linux tray menu MUST NOT use bare process/memory header without badge (AC-007.63)"
    );
}

/// FR-007 / AC-007.62 — Swift NSStatusItem title leads with gate visual badge_label.
#[test]
fn fr007_tray_thermal_visual_swift_menubar_title_wires_gate_visual() {
    let op = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift");
    assert!(
        op.contains("formatMenuBarTitleLine"),
        "Swift OperatorDisplay MUST expose menu bar title formatter (AC-007.62)"
    );
    assert!(
        op.contains("formatMenuBarTitleOfflineLine"),
        "Swift OperatorDisplay MUST expose offline menu bar title formatter (AC-007.62)"
    );

    let app = include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/AppEntry.swift");
    assert!(
        app.contains("formatMenuBarTitleLine"),
        "AppEntry MUST bind status item title from gate visual (AC-007.62)"
    );
    assert!(
        app.contains("formatMenuBarTitleOfflineLine"),
        "AppEntry MUST bind offline status item title from gate visual (AC-007.62)"
    );
    assert!(
        op.contains("visual.badgeLabel"),
        "Swift menu bar title formatter MUST lead with gate visual badge (AC-007.62)"
    );
    assert!(
        !app.contains("managed_processes) | \\(snap.used_memory_mb)M"),
        "AppEntry MUST NOT use bare managed/memory title without badge (AC-007.62)"
    );
    assert!(
        !app.contains("title = \" offline\""),
        "AppEntry MUST NOT use generic offline title without badge (AC-007.62)"
    );
}

/// FR-007 / AC-007.60 — Windows HealthStatusText uses gate visual tokens.
#[test]
fn fr007_tray_thermal_visual_windows_health_status_wires_gate_visual() {
    let op = include_str!("../windows/ShareCLITray/OperatorDisplay.cs");
    assert!(
        op.contains("FormatHealthStatusLine"),
        "C# OperatorDisplay MUST expose health summary formatter (AC-007.60)"
    );
    assert!(
        op.contains("FormatHealthStatusOfflineLine"),
        "C# OperatorDisplay MUST expose offline health summary formatter (AC-007.60)"
    );

    let code = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        code.contains("FormatHealthStatusLine"),
        "TrayWindow MUST bind HealthStatusText from gate visual (AC-007.60)"
    );
    assert!(
        code.contains("HealthStatusText.Foreground"),
        "TrayWindow MUST color HealthStatusText by severity (AC-007.60)"
    );
    assert!(
        !code.contains("health.Healthy ? \"✓ OK\""),
        "TrayWindow MUST NOT use generic OK/Unhealthy health summary (AC-007.60)"
    );
}
