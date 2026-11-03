// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — IPC health/pool/status embedded pool + status siblings (AC-007.78)
//! FR: FR-007
//!
//! IPC `health.status`, `pool.status`, and `status.snapshot` embed operator `pool` /
//! `status` siblings after `gate` → `host_watch` (parity with CLI `--json` AC-007.77).

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

fn assert_pool_object(pool: &serde_json::Value, context: &str) {
    assert!(
        pool.get("node_total").is_some() && pool.get("healthy").is_some(),
        "{context} pool MUST include capacity fields (AC-007.78); got: {pool}"
    );
}

fn assert_status_object(status: &serde_json::Value, context: &str) {
    assert!(
        status.get("total_processes").is_some()
            && status.get("scanned").is_some()
            && status.get("watched").is_some(),
        "{context} status MUST include proc-scan fields (AC-007.78); got: {status}"
    );
}

fn assert_json_gate_host_watch_pool_status_order(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.78)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.78)");
    let pool = v.get("pool").expect("{context} MUST include pool (AC-007.78)");
    let status = v.get("status").expect("{context} MUST include status (AC-007.78)");
    assert_pool_object(pool, context);
    assert_status_object(status, context);
    assert!(gate.get("gate_decision").is_some(), "gate MUST include gate_decision (AC-007.78)");
    assert!(host.get("load_1m").is_some(), "host_watch MUST include load_1m (AC-007.78)");

    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let pool_pos = raw.find("\"pool\"").expect("pool key in raw JSON (AC-007.78)");
    let status_pos = raw.find("\"status\"").expect("status key in raw JSON (AC-007.78)");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.78); got: {raw}"
    );
}

fn assert_json_gate_host_watch_status_only(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(
        v.get("node_total").is_some(),
        "{context} top-level MUST include pool panel fields (AC-007.78); got: {v}"
    );
    let status = v.get("status").expect("{context} MUST include nested status (AC-007.78)");
    assert_status_object(status, context);
    assert!(
        v.get("pool").is_none(),
        "{context} MUST NOT include redundant nested pool (AC-007.78)"
    );

    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let status_pos = raw.rfind("\"status\"").expect("status key in raw JSON (AC-007.78)");
    assert!(
        host_pos < status_pos,
        "{context} MUST serialize host_watch before nested status (AC-007.78); got: {raw}"
    );
}

fn assert_json_gate_host_watch_pool_only(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    assert!(
        v.get("total_processes").is_some(),
        "{context} top-level MUST include proc-scan panel fields (AC-007.78); got: {v}"
    );
    let pool = v.get("pool").expect("{context} MUST include nested pool (AC-007.78)");
    assert_pool_object(pool, context);
    assert!(
        pool.get("status").is_none(),
        "nested pool MUST NOT include cross-sibling status (AC-007.78)"
    );

    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let pool_pos = raw.rfind("\"pool\"").expect("pool key in raw JSON (AC-007.78)");
    assert!(
        host_pos < pool_pos,
        "{context} MUST serialize host_watch before nested pool (AC-007.78); got: {raw}"
    );
}

/// FR-007 / AC-007.78 — live `health.status` dispatch embeds pool + status siblings.
#[tokio::test]
async fn fr007_ipc_health_status_pool_status_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"health.status","params":{}}"#).await;
    assert!(resp.error.is_none(), "health.status MUST succeed (AC-007.78); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize health.status result");
    assert_json_gate_host_watch_pool_status_order(&raw, "health.status");
}

/// FR-007 / AC-007.78 — live `pool.status` dispatch embeds nested status only.
#[tokio::test]
async fn fr007_ipc_pool_status_nested_status_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":2,"method":"pool.status","params":{}}"#).await;
    assert!(resp.error.is_none(), "pool.status MUST succeed (AC-007.78); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize pool.status result");
    assert_json_gate_host_watch_status_only(&raw, "pool.status");
}

/// FR-007 / AC-007.78 — live `status.snapshot` dispatch embeds nested pool only.
#[tokio::test]
async fn fr007_ipc_status_snapshot_nested_pool_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":3,"method":"status.snapshot","params":{}}"#).await;
    assert!(resp.error.is_none(), "status.snapshot MUST succeed (AC-007.78); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize status.snapshot result");
    assert_json_gate_host_watch_pool_only(&raw, "status.snapshot");
}

/// FR-007 / AC-007.78 — serialized HealthSnapshot preserves operator key order.
#[test]
fn fr007_ipc_health_snapshot_pool_status_order() {
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;
    use sharecli_ipc::handler::{HealthSnapshot, PoolSnapshot, StatusSnapshot};

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
    let snap = HealthSnapshot {
        managed_processes: 2,
        used_memory_mb: 512,
        total_memory_mb: 16384,
        healthy: true,
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
    let json = serde_json::to_string(&snap).expect("serialize HealthSnapshot");
    assert_json_gate_host_watch_pool_status_order(&json, "HealthSnapshot");
}

/// FR-007 / AC-007.78 — tray wire shape decodes embedded pool + status from health.status JSON.
#[test]
fn fr007_ipc_health_snapshot_wire_pool_status_roundtrip() {
    use sharecli_ipc::handler::HealthSnapshot;

    let raw = format!(
        r#"{{"managed_processes":3,"used_memory_mb":2048,"total_memory_mb":16384,
        "healthy":true,"gate":{{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"}},
        "host_watch":{{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}},{SAMPLE_POOL_TAIL},{SAMPLE_STATUS_TAIL}}}"#
    );
    let h: HealthSnapshot = serde_json::from_str(&raw).expect("decode HealthSnapshot wire JSON");
    assert_eq!(h.managed_processes, 3);
    assert_eq!(h.pool.node_total, 2);
    assert_eq!(h.status.scanned, 50);
    assert_json_gate_host_watch_pool_status_order(&raw, "HealthSnapshot wire");
}
