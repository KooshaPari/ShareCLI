// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — tray gate/host_watch UI display parity (AC-007.56)
//! FR: FR-007
//!
//! Linux, Swift, and Windows tray UIs MUST surface thermal gate + host_watch operator
//! metadata from `monitoring.report` snapshots using shared format helpers.

use sharecli_tray_linux::ipc::{GateStatusSnapshot, HostResourceWatchJson};
use sharecli_tray_linux::operator_display as linux_display;
use sharecli_tray_windows::operator_display as win_display;

fn sample_gate() -> GateStatusSnapshot {
    GateStatusSnapshot {
        thermal_pressure: "YELLOW".into(),
        detected_agents: 1,
        agent_total_rss_bytes: 512,
        agent_contention: "WARN".into(),
        gate_decision: "THROTTLE".into(),
    }
}

fn sample_host() -> HostResourceWatchJson {
    HostResourceWatchJson {
        fd_count: 10,
        net_rx_bytes: 100,
        net_tx_bytes: 200,
        mem_rss_bytes: 4096,
        load_1m: 1.25,
    }
}

/// FR-007 / AC-007.56 — Linux tray format helpers map gate → host_watch key fields.
#[test]
fn fr007_tray_gate_host_watch_ui_linux_format() {
    let gate = sample_gate();
    let host = sample_host();

    let gate_line = linux_display::format_gate_tray_line(&gate);
    assert_eq!(gate_line, "Gate [THROTTLE] · YELLOW · agents 1 · WARN");

    let rss_line = linux_display::format_gate_rss_tray_line(&gate);
    assert_eq!(rss_line, "Agent RSS: 512 B");

    let watch_line = linux_display::format_host_watch_tray_line(&host);
    assert_eq!(watch_line, "Host load 1.25 · FDs 10 · RSS 4.0 KB");

    let net_line = linux_display::format_host_net_tray_line(&host);
    assert_eq!(net_line, "Net RX 100 B · TX 200 B");

    let lines = linux_display::format_operator_tray_lines(&gate, &host);
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("Gate ["));
    assert!(lines[2].starts_with("Host load"));
}

/// FR-007 / AC-007.56 — Windows tray Rust helpers stay byte-identical to Linux.
#[test]
fn fr007_tray_gate_host_watch_ui_windows_linux_parity() {
    use sharecli_tray_windows::ipc::{
        GateStatusSnapshot as WinGate, HostResourceWatchJson as WinHost,
    };

    let linux_gate = sample_gate();
    let linux_host = sample_host();

    let win_gate = WinGate {
        thermal_pressure: linux_gate.thermal_pressure.clone(),
        detected_agents: linux_gate.detected_agents,
        agent_total_rss_bytes: linux_gate.agent_total_rss_bytes,
        agent_contention: linux_gate.agent_contention.clone(),
        gate_decision: linux_gate.gate_decision.clone(),
    };
    let win_host = WinHost {
        fd_count: linux_host.fd_count,
        net_rx_bytes: linux_host.net_rx_bytes,
        net_tx_bytes: linux_host.net_tx_bytes,
        mem_rss_bytes: linux_host.mem_rss_bytes,
        load_1m: linux_host.load_1m,
    };

    assert_eq!(
        linux_display::format_gate_tray_line(&linux_gate),
        win_display::format_gate_tray_line(&win_gate),
    );
    assert_eq!(
        linux_display::format_host_watch_tray_line(&linux_host),
        win_display::format_host_watch_tray_line(&win_host),
    );
    assert_eq!(
        linux_display::format_operator_status_summary(&linux_gate, &linux_host),
        win_display::format_operator_status_summary(&win_gate, &win_host),
    );
}

/// FR-007 / AC-007.56 — Linux tray menu surfaces operator lines from monitoring.report health.
#[test]
fn fr007_tray_gate_host_watch_ui_linux_main_wires_operator_display() {
    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("operator_display::format_operator_tray_lines"),
        "Linux tray menu MUST render gate/host_watch lines (AC-007.56)"
    );
    assert!(
        main_rs.contains("operator_display::format_operator_status_summary"),
        "Linux tray tooltip MUST include operator summary (AC-007.56)"
    );
}

/// FR-007 / AC-007.56 — Swift OperatorDisplay + popover/health views wire tray strings.
#[test]
fn fr007_tray_gate_host_watch_ui_swift_wires_operator_display() {
    let op = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift");
    assert!(
        op.contains("formatOperatorTrayLines"),
        "Swift OperatorDisplay MUST expose tray line formatter (AC-007.56)"
    );
    assert!(
        op.contains("Gate [\\(gate.gate_decision)]"),
        "Swift gate line MUST include decision + thermal + agents + contention (AC-007.56)"
    );

    let popover =
        include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift");
    assert!(
        popover.contains("OperatorDisplay.formatOperatorTrayLines"),
        "Tray popover MUST surface gate/host_watch lines (AC-007.56)"
    );

    // The dashboard Health view moved from DashboardView.swift to HealthPage.swift
    // (3-subpage layout: Memory / Thermal gate / Host watch).
    let health = include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/HealthPage.swift");
    assert!(
        health.contains("HostWatchSubpage"),
        "Dashboard Health view MUST surface host_watch fields (AC-007.56)"
    );
    assert!(
        health.contains("resolveTrayGateVisual"),
        "Dashboard Health view MUST surface gate fields (AC-007.56)"
    );
}

/// FR-007 / AC-007.56 — Windows WinUI tray surfaces gate + host_watch operator rows.
#[test]
fn fr007_tray_gate_host_watch_ui_windows_wires_operator_display() {
    let op = include_str!("../windows/ShareCLITray/OperatorDisplay.cs");
    assert!(
        op.contains("FormatOperatorTrayLines"),
        "C# OperatorDisplay MUST expose tray line formatter (AC-007.56)"
    );

    let xaml = include_str!("../windows/ShareCLITray/TrayWindow.xaml");
    assert!(
        xaml.contains("GateStatusText"),
        "TrayWindow XAML MUST include gate status row (AC-007.56)"
    );
    assert!(
        xaml.contains("HostWatchStatusText"),
        "TrayWindow XAML MUST include host_watch status row (AC-007.56)"
    );

    let code = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        code.contains("OperatorDisplay.FormatGateTrayLine"),
        "TrayWindow MUST bind gate operator strings (AC-007.56)"
    );
    assert!(
        code.contains("OperatorDisplay.FormatHostWatchTrayLine"),
        "TrayWindow MUST bind host_watch operator strings (AC-007.56)"
    );
}

/// FR-007 / AC-007.56 — monitoring.report mapping feeds tray format inputs.
#[test]
fn fr007_tray_gate_host_watch_ui_maps_from_monitoring_report() {
    use sharecli_tray_linux::ipc::MonitoringReportSnapshot;

    let raw = r#"{"timestamp":1700000000,"total_processes":1,"used_memory_mb":128,
        "total_memory_mb":8192,"processes":[{"pid":42,"name":"svc","memory_mb":64,
        "project":null,"harness":"native"}],
        "gate":{"thermal_pressure":"YELLOW","detected_agents":1,
        "agent_total_rss_bytes":512,"agent_contention":"WARN","gate_decision":"THROTTLE"},
        "host_watch":{"fd_count":10,"net_rx_bytes":100,"net_tx_bytes":200,
        "mem_rss_bytes":4096,"load_1m":1.25},
        "pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}},
        "status":{"total_processes":2,"agents":[],"scanned":50,"watched":1,
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}}"#;
    let snap: MonitoringReportSnapshot = serde_json::from_str(raw).unwrap();
    let health = snap.health_snapshot();

    let lines = linux_display::format_operator_tray_lines(&health.gate, &health.host_watch);
    assert_eq!(lines[0], "Gate [THROTTLE] · YELLOW · agents 1 · WARN");
    assert_eq!(lines[2], "Host load 1.25 · FDs 10 · RSS 4.0 KB");
}
