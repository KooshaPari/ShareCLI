// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Windows tray process grid harness column parity (AC-007.55)
//! FR: FR-007
//!
//! WinUI tray MUST map `monitoring.report` process `harness` into tray rows and surface it in
//! the process DataGrid (parity with Linux tray submenu Harness label and Swift Harness column).

use sharecli_tray_windows::ipc::MonitoringReportSnapshot;

/// FR-007 / AC-007.55 — Rust process_summaries preserves harness from monitoring.report wire.
#[test]
fn fr007_tray_windows_harness_maps_from_monitoring_report() {
    let raw = r#"{"timestamp":1700000000,"total_processes":2,"used_memory_mb":256,
        "total_memory_mb":16384,"processes":[{"pid":99,"name":"worker","memory_mb":64,
        "project":"demo","harness":"claude"},{"pid":100,"name":"agent","memory_mb":32,
        "project":null,"harness":null}],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5},
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
    let procs = snap.process_summaries();
    assert_eq!(procs[0].harness.as_deref(), Some("claude"));
    assert!(procs[1].harness.is_none());
}

/// FR-007 / AC-007.55 — C# AsProcessSummaries maps MonitoringProcessEntry.harness → ProcessInfo.
#[test]
fn fr007_tray_windows_harness_csharp_mapping_wires() {
    let snap_cs = include_str!("../windows/ShareCLITray/MonitoringReportSnapshot.cs");
    assert!(
        snap_cs.contains("harness = entry.Harness"),
        "AsProcessSummaries MUST map harness (AC-007.55)"
    );

    let tray_cs = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        tray_cs.contains("public string? harness"),
        "ProcessInfo MUST expose harness field (AC-007.55)"
    );
}

/// FR-007 / AC-007.55 — WinUI grid binds Harness column for operator process metadata.
#[test]
fn fr007_tray_windows_harness_grid_column_wires() {
    let tray_xaml = include_str!("../windows/ShareCLITray/TrayWindow.xaml");
    // The tray grid is built from TextBlock cells (Grid.Column) rather than a
    // DataGridTextColumn, so the header/binding appear as Text="{Binding harness}".
    assert!(
        tray_xaml.contains("Text=\"Harness\""),
        "ProcessGrid MUST include Harness column (AC-007.55)"
    );
    assert!(
        tray_xaml.contains("Text=\"{Binding harness}\""),
        "Harness column MUST bind ProcessInfo.harness (AC-007.55)"
    );
}

/// FR-007 / AC-007.55 — Linux tray submenu surfaces harness as reference contract.
#[test]
fn fr007_tray_windows_harness_linux_reference_parity() {
    let linux_main = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        linux_main.contains("Harness:"),
        "Linux tray MUST label harness in process submenu (AC-007.55 reference)"
    );
    assert!(
        linux_main.contains("proc.harness"),
        "Linux tray MUST read harness from ProcessSummary (AC-007.55 reference)"
    );
}
