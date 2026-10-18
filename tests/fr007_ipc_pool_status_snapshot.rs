// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — IPC `pool.status` + `status.snapshot` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.67 `pool.status` / `PoolSnapshot` and `status.snapshot` / `StatusSnapshot`
//! emit top-level `gate` + `host_watch` siblings (parity with `pool --json` AC-007.44 and
//! `status --json` AC-007.25) for tray/desktop and automation consumers.

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
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.67); got: {host}");
    }
}

fn assert_gate_object(gate: &serde_json::Value) {
    for key in GATE_KEYS {
        assert!(gate.get(key).is_some(), "gate MUST include {key} (AC-007.67); got: {gate}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.67)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.67)");
    assert_gate_object(gate);
    assert_host_watch_object(host);
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.67); got: {raw}"
    );
}

/// FR-007 / AC-007.67 — live `pool.status` dispatch carries gate + host_watch siblings.
#[tokio::test]
async fn fr007_ipc_pool_status_gate_host_watch_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"pool.status","params":{}}"#).await;
    assert!(resp.error.is_none(), "pool.status MUST succeed (AC-007.67); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize pool.status result");
    assert_json_gate_before_host_watch(&raw, "pool.status");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(
        v.get("node_total").is_some() && v.get("bun_total").is_some(),
        "pool.status MUST include pool fields (AC-007.67)"
    );
    assert!(v.get("healthy").is_some(), "pool.status MUST include healthy (AC-007.67)");
}

/// FR-007 / AC-007.67 — live `status.snapshot` dispatch carries gate + host_watch siblings.
#[tokio::test]
async fn fr007_ipc_status_snapshot_gate_host_watch_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":2,"method":"status.snapshot","params":{}}"#).await;
    assert!(resp.error.is_none(), "status.snapshot MUST succeed (AC-007.67); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize status.snapshot result");
    assert_json_gate_before_host_watch(&raw, "status.snapshot");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(
        v.get("total_processes").is_some(),
        "status.snapshot MUST include total_processes (AC-007.67)"
    );
    assert!(v.get("agents").is_some(), "status.snapshot MUST include agents (AC-007.67)");
    assert!(
        v.get("scanned").is_some() && v.get("watched").is_some(),
        "status.snapshot MUST include scanned/watched (AC-007.67)"
    );
}

/// FR-007 / AC-007.67 — serialized PoolSnapshot preserves gate → host_watch key order.
#[test]
fn fr007_ipc_pool_snapshot_gate_before_host_watch() {
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;
    use sharecli_ipc::handler::PoolSnapshot;

    let snap = PoolSnapshot {
        node_total: 2,
        node_idle: 1,
        bun_total: 1,
        bun_idle: 0,
        max_per_type: 4,
        healthy: true,
        issues: vec![],
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        },
        host_watch: HostResourceWatchJson {
            fd_count: 10,
            net_rx_bytes: 100,
            net_tx_bytes: 200,
            mem_rss_bytes: 4096,
            load_1m: 0.42,
        },
        status: None,
    };
    let json = serde_json::to_string(&snap).expect("serialize PoolSnapshot");
    assert_json_gate_before_host_watch(&json, "PoolSnapshot");
}

/// FR-007 / AC-007.67 — serialized StatusSnapshot preserves gate → host_watch key order.
#[test]
fn fr007_ipc_status_snapshot_gate_before_host_watch() {
    use sharecli::commands::proc::AgentProcRow;
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;
    use sharecli_ipc::handler::StatusSnapshot;

    let snap = StatusSnapshot {
        total_processes: 3,
        agents: vec![AgentProcRow {
            pid: 42,
            family: "claude".into(),
            comm: "claude".into(),
            state: "S".into(),
            mem_rss_bytes: 1024,
            mem_rss: "1.0M".into(),
            fd_count: Some(8),
        }],
        scanned: 100,
        watched: 1,
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 1,
            agent_total_rss_bytes: 1024,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        },
        host_watch: HostResourceWatchJson {
            fd_count: 10,
            net_rx_bytes: 100,
            net_tx_bytes: 200,
            mem_rss_bytes: 4096,
            load_1m: 0.42,
        },
        pool: None,
    };
    let json = serde_json::to_string(&snap).expect("serialize StatusSnapshot");
    assert_json_gate_before_host_watch(&json, "StatusSnapshot");
}

/// FR-007 / AC-007.67 — tray wire shape decodes gate + host_watch from pool.status JSON.
#[test]
fn fr007_ipc_pool_snapshot_wire_roundtrip() {
    use sharecli_ipc::handler::PoolSnapshot;

    let raw = r#"{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: PoolSnapshot = serde_json::from_str(raw).expect("decode PoolSnapshot wire JSON");
    assert_eq!(snap.node_total, 2);
    assert_eq!(snap.bun_idle, 0);
    assert!(snap.healthy);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "PoolSnapshot wire");
}

/// FR-007 / AC-007.67 — tray wire shape decodes gate + host_watch from status.snapshot JSON.
#[test]
fn fr007_ipc_status_snapshot_wire_roundtrip() {
    use sharecli_ipc::handler::StatusSnapshot;

    let raw = r#"{"total_processes":2,"agents":[{"pid":99,"family":"claude","comm":"claude",
        "state":"S","mem_rss_bytes":4096,"mem_rss":"4.0M","fd_count":12}],
        "scanned":50,"watched":1,
        "gate":{"thermal_pressure":"GREEN","detected_agents":1,
        "agent_total_rss_bytes":4096,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: StatusSnapshot = serde_json::from_str(raw).expect("decode StatusSnapshot wire JSON");
    assert_eq!(snap.total_processes, 2);
    assert_eq!(snap.agents[0].pid, 99);
    assert_eq!(snap.scanned, 50);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "StatusSnapshot wire");
}
