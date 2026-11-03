// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — IPC `monitoring.report` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.46 `monitoring.report` / `MonitoringReportSnapshot` emit top-level `gate` +
//! `host_watch` siblings (parity with `report --format json` AC-007.40 and
//! `health.status` AC-007.45) for tray/desktop consumers.

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

const GATE_KEYS: [&str; 5] = [
    "thermal_pressure",
    "detected_agents",
    "agent_total_rss_bytes",
    "agent_contention",
    "gate_decision",
];

fn assert_host_watch_object(host: &serde_json::Value) {
    for key in HOST_WATCH_KEYS {
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.46); got: {host}");
    }
}

fn assert_gate_object(gate: &serde_json::Value) {
    for key in GATE_KEYS {
        assert!(gate.get(key).is_some(), "gate MUST include {key} (AC-007.46); got: {gate}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.46)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.46)");
    assert_gate_object(gate);
    assert_host_watch_object(host);
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.46); got: {raw}"
    );
}

/// FR-007 / AC-007.46 — live `monitoring.report` dispatch carries gate + host_watch siblings.
#[tokio::test]
async fn fr007_ipc_monitoring_report_gate_host_watch_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"monitoring.report","params":{}}"#).await;
    assert!(
        resp.error.is_none(),
        "monitoring.report MUST succeed (AC-007.46); err={:?}",
        resp.error
    );
    let raw = serde_json::to_string(&resp.result).expect("serialize monitoring.report result");
    assert_json_gate_before_host_watch(&raw, "monitoring.report");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(v.get("timestamp").is_some(), "monitoring.report MUST include timestamp (AC-007.46)");
    assert!(
        v.get("total_processes").is_some(),
        "monitoring.report MUST include total_processes (AC-007.46)"
    );
    assert!(v.get("processes").is_some(), "monitoring.report MUST include processes (AC-007.46)");
}

/// FR-007 / AC-007.46 — serialized MonitoringReportSnapshot preserves gate → host_watch key order.
#[test]
fn fr007_ipc_monitoring_report_snapshot_gate_before_host_watch() {
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;
    use sharecli_ipc::handler::{
        MonitoringProcessEntry, MonitoringReportSnapshot, PoolSnapshot, StatusSnapshot,
    };

    let gate = GateStatusSnapshot {
        thermal_pressure: "GREEN".into(),
        detected_agents: 1,
        agent_total_rss_bytes: 512,
        agent_contention: "OK".into(),
        gate_decision: "ADMIT".into(),
    };
    let host_watch = HostResourceWatchJson {
        fd_count: 10,
        net_rx_bytes: 100,
        net_tx_bytes: 200,
        mem_rss_bytes: 4096,
        load_1m: 0.42,
    };
    let snap = MonitoringReportSnapshot {
        timestamp: 1_700_000_000,
        total_processes: 2,
        used_memory_mb: 512,
        total_memory_mb: 16384,
        processes: vec![MonitoringProcessEntry {
            pid: 42,
            name: "agent".into(),
            memory_mb: 128,
            project: Some("demo".into()),
            harness: None,
            start_time: 0,
            cpu_percent: 0.0,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: "Unknown".into(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        }],
        gate: gate.clone(),
        host_watch,
        pool: PoolSnapshot {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
            issues: vec![],
            gate: gate.clone(),
            host_watch,
            status: None,
        },
        status: StatusSnapshot {
            total_processes: 2,
            agents: vec![],
            scanned: 50,
            watched: 1,
            gate,
            host_watch,
            pool: None,
        },
    };
    let json = serde_json::to_string(&snap).expect("serialize MonitoringReportSnapshot");
    assert_json_gate_before_host_watch(&json, "MonitoringReportSnapshot");
}

/// FR-007 / AC-007.47 — tray wire shape decodes gate + host_watch from monitoring.report JSON.
#[test]
fn fr007_ipc_monitoring_report_tray_wire_roundtrip() {
    // Mirrors sharecli-tray-linux `monitoring_report_snapshot_matches_server_wire_shape`.
    use sharecli_ipc::handler::MonitoringReportSnapshot;

    let raw = r#"{"timestamp":1700000000,"total_processes":1,"used_memory_mb":256,
        "total_memory_mb":16384,"processes":[{"pid":99,"name":"worker","memory_mb":64,
        "project":null,"harness":"native"}],
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
    let snap: MonitoringReportSnapshot =
        serde_json::from_str(raw).expect("decode MonitoringReportSnapshot wire JSON");
    assert_eq!(snap.total_processes, 1);
    assert_eq!(snap.processes[0].pid, 99);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "MonitoringReportSnapshot wire");
}
