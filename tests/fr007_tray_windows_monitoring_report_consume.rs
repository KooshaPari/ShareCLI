// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Windows tray refresh consumes `monitoring.report` snapshot (AC-007.51)
//! FR: FR-007
//!
//! WinUI tray refresh MUST drive operator gate/host_watch + process inventory from one
//! `monitoring.report` IPC round-trip (parity with Linux/Swift AC-007.48), not split
//! `health.status` + `process.list` polls.

/// FR-007 / AC-007.51 — live `monitoring.report` dispatch feeds Windows tray mapping helpers.
#[tokio::test]
async fn fr007_tray_windows_monitoring_report_refresh_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"monitoring.report","params":{}}"#).await;
    assert!(
        resp.error.is_none(),
        "monitoring.report MUST succeed (AC-007.51); err={:?}",
        resp.error
    );

    let raw = serde_json::to_string(&resp.result).expect("serialize monitoring.report result");
    let snap: sharecli_tray_windows::ipc::MonitoringReportSnapshot =
        serde_json::from_str(&raw).expect("decode MonitoringReportSnapshot for Windows tray");
    let health = snap.health_snapshot();
    let procs = snap.process_summaries();

    assert_eq!(health.managed_processes, snap.total_processes);
    assert_eq!(health.used_memory_mb, snap.used_memory_mb);
    assert_eq!(health.total_memory_mb, snap.total_memory_mb);
    assert_eq!(health.gate.gate_decision, snap.gate.gate_decision);
    assert_eq!(health.host_watch.load_1m, snap.host_watch.load_1m);
    assert_eq!(procs.len(), snap.processes.len());
}

/// FR-007 / AC-007.51 — Windows tray mapping preserves gate → host_watch from monitoring.report wire.
#[test]
fn fr007_tray_windows_monitoring_report_maps_gate_host_watch_order() {
    use sharecli_tray_windows::ipc::MonitoringReportSnapshot;

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
    let gate_pos = raw.find("\"gate\"").expect("gate in wire JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch in wire JSON");
    assert!(gate_pos < host_pos, "gate MUST precede host_watch on wire (AC-007.51)");

    let snap: MonitoringReportSnapshot = serde_json::from_str(raw).unwrap();
    let health = snap.health_snapshot();
    assert_eq!(health.gate.gate_decision, "THROTTLE");
    assert_eq!(health.host_watch.load_1m, 1.25);
    assert_eq!(snap.process_summaries()[0].pid, 42);
    assert_eq!(
        snap.process_summaries()[0].harness.as_deref(),
        Some("native"),
        "process_summaries MUST map harness from monitoring.report (AC-007.55)"
    );
}
