// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — IPC `monitoring.report` embedded pool + status siblings (AC-007.72)
//! FR: FR-007
//!
//! `monitoring.report` / `MonitoringReportSnapshot` embed top-level `pool` + `status`
//! after `gate` → `host_watch` (parity with dashboard WS AC-007.70) so tray refresh
//! avoids supplementary `pool.status` / `status.snapshot` round-trips.

const SAMPLE_POOL_TAIL: &str = r#""pool":{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
            "healthy":true,"issues":[],
            "gate":{"thermal_pressure":"GREEN","detected_agents":0,
            "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}"#;

const SAMPLE_STATUS_TAIL: &str = r#""status":{"total_processes":2,"agents":[{"pid":99,"family":"claude","comm":"claude",
            "state":"S","mem_rss_bytes":4096,"mem_rss":"4.0M","fd_count":12}],
            "scanned":50,"watched":1,
            "gate":{"thermal_pressure":"GREEN","detected_agents":1,
            "agent_total_rss_bytes":4096,"agent_contention":"OK","gate_decision":"ADMIT"},
            "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
            "mem_rss_bytes":4,"load_1m":0.5}}"#;

fn assert_json_gate_host_watch_pool_status_order(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.72)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.72)");
    let pool = v.get("pool").expect("{context} MUST include pool (AC-007.72)");
    let status = v.get("status").expect("{context} MUST include status (AC-007.72)");
    assert!(
        pool.get("node_total").is_some() && pool.get("healthy").is_some(),
        "pool MUST include capacity fields (AC-007.72); got: {pool}"
    );
    assert!(
        status.get("total_processes").is_some()
            && status.get("scanned").is_some()
            && status.get("watched").is_some(),
        "status MUST include proc-scan fields (AC-007.72); got: {status}"
    );
    assert!(gate.get("gate_decision").is_some(), "gate MUST include gate_decision (AC-007.72)");
    assert!(host.get("load_1m").is_some(), "host_watch MUST include load_1m (AC-007.72)");

    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let pool_pos = raw.find("\"pool\"").expect("pool key in raw JSON (AC-007.72)");
    let status_pos = raw.find("\"status\"").expect("status key in raw JSON (AC-007.72)");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.72); got: {raw}"
    );
}

/// FR-007 / AC-007.72 — live `monitoring.report` dispatch embeds pool + status siblings.
#[tokio::test]
async fn fr007_ipc_monitoring_report_pool_status_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"monitoring.report","params":{}}"#).await;
    assert!(
        resp.error.is_none(),
        "monitoring.report MUST succeed (AC-007.72); err={:?}",
        resp.error
    );
    let raw = serde_json::to_string(&resp.result).expect("serialize monitoring.report result");
    assert_json_gate_host_watch_pool_status_order(&raw, "monitoring.report");
}

/// FR-007 / AC-007.72 — serialized MonitoringReportSnapshot preserves operator key order.
#[test]
fn fr007_ipc_monitoring_report_snapshot_pool_status_order() {
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
    assert_json_gate_host_watch_pool_status_order(&json, "MonitoringReportSnapshot");
}

/// FR-007 / AC-007.72 — tray wire shape decodes embedded pool + status from monitoring.report JSON.
#[test]
fn fr007_ipc_monitoring_report_tray_wire_pool_status_roundtrip() {
    use sharecli_ipc::handler::MonitoringReportSnapshot;

    let raw = format!(
        r#"{{"timestamp":1700000000,"total_processes":1,"used_memory_mb":256,
        "total_memory_mb":16384,"processes":[{{"pid":99,"name":"worker","memory_mb":64,
        "project":null,"harness":"native"}}],
        "gate":{{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"}},
        "host_watch":{{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}},{SAMPLE_POOL_TAIL},{SAMPLE_STATUS_TAIL}}}"#
    );
    let snap: MonitoringReportSnapshot =
        serde_json::from_str(&raw).expect("decode MonitoringReportSnapshot wire JSON");
    assert_eq!(snap.pool.node_total, 2);
    assert_eq!(snap.status.scanned, 50);
    assert_json_gate_host_watch_pool_status_order(&raw, "MonitoringReportSnapshot wire");
}
