// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — IPC `health.status` gate + host_watch siblings
//! FR: FR-007
//!
//! AC-007.45 `health.status` / `HealthSnapshot` emit top-level `gate` + `host_watch`
//! siblings (parity with `health --json` AC-007.44) for tray/desktop consumers.

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
        assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.45); got: {host}");
    }
}

fn assert_gate_object(gate: &serde_json::Value) {
    for key in GATE_KEYS {
        assert!(gate.get(key).is_some(), "gate MUST include {key} (AC-007.45); got: {gate}");
    }
}

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.45)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.45)");
    assert_gate_object(gate);
    assert_host_watch_object(host);
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.45); got: {raw}"
    );
}

/// FR-007 / AC-007.45 — live `health.status` dispatch carries gate + host_watch siblings.
#[tokio::test]
async fn fr007_ipc_health_status_gate_host_watch_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"health.status","params":{}}"#).await;
    assert!(resp.error.is_none(), "health.status MUST succeed (AC-007.45); err={:?}", resp.error);
    let raw = serde_json::to_string(&resp.result).expect("serialize health.status result");
    assert_json_gate_before_host_watch(&raw, "health.status");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(v.get("healthy").is_some(), "health.status MUST include healthy (AC-007.45)");
    assert!(
        v.get("managed_processes").is_some(),
        "health.status MUST include managed_processes (AC-007.45)"
    );
}

/// FR-007 / AC-007.45 — serialized HealthSnapshot preserves gate → host_watch key order.
#[test]
fn fr007_ipc_health_snapshot_gate_before_host_watch() {
    use sharecli::monitoring::HostResourceWatchJson;
    use sharecli_fleet::GateStatusSnapshot;
    use sharecli_ipc::handler::HealthSnapshot;

    let snap = HealthSnapshot {
        managed_processes: 2,
        used_memory_mb: 1024,
        total_memory_mb: 16384,
        healthy: true,
        gate: GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 1,
            agent_total_rss_bytes: 512,
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
        pool: sharecli_ipc::handler::PoolSnapshot {
            node_total: 0,
            node_idle: 0,
            bun_total: 0,
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
                fd_count: 0,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 0,
                load_1m: 0.0,
            },
            status: None,
        },
        status: sharecli_ipc::handler::StatusSnapshot {
            total_processes: 0,
            agents: vec![],
            scanned: 0,
            watched: 0,
            gate: GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson {
                fd_count: 0,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 0,
                load_1m: 0.0,
            },
            pool: None,
        },
    };
    let json = serde_json::to_string(&snap).expect("serialize HealthSnapshot");
    assert_json_gate_before_host_watch(&json, "HealthSnapshot");
}

/// FR-007 / AC-007.45 — tray wire shape decodes gate + host_watch from health.status JSON.
#[test]
fn fr007_ipc_health_snapshot_wire_roundtrip() {
    use sharecli_ipc::handler::HealthSnapshot;

    let raw = r#"{"managed_processes":3,"used_memory_mb":2048,"total_memory_mb":16384,
        "healthy":true,"gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5},
        "pool":{"node_total":0,"node_idle":0,"bun_total":0,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":0,"net_rx_bytes":0,"net_tx_bytes":0,
        "mem_rss_bytes":0,"load_1m":0.0}},
        "status":{"total_processes":0,"agents":[],"scanned":0,"watched":0,
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":0,"net_rx_bytes":0,"net_tx_bytes":0,
        "mem_rss_bytes":0,"load_1m":0.0}}}"#
        .to_string();
    let h: HealthSnapshot = serde_json::from_str(&raw).expect("decode HealthSnapshot wire JSON");
    assert_eq!(h.managed_processes, 3);
    assert_eq!(h.used_memory_mb, 2048);
    assert!(h.healthy);
    assert_eq!(h.gate.gate_decision, "ADMIT");
    assert_eq!(h.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(&raw, "HealthSnapshot wire");
}
