// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — WebSocket `health_update` wire decodes expanded HealthSnapshot pool/status (AC-007.80)
//! FR: FR-007
//!
//! IPC `health.status` (AC-007.78) and CLI `--json` (AC-007.77) embed operator pool/status siblings.
//! The typed WS client MUST decode `ClientMessage::HealthUpdate` from the expanded envelope rather
//! than falling through to `ClientMessage::Unknown`.

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
        "{context} pool MUST include capacity fields (AC-007.80); got: {pool}"
    );
}

fn assert_status_object(status: &serde_json::Value, context: &str) {
    assert!(
        status.get("total_processes").is_some()
            && status.get("scanned").is_some()
            && status.get("watched").is_some(),
        "{context} status MUST include proc-scan fields (AC-007.80); got: {status}"
    );
}

fn assert_health_gate_host_watch_pool_status_order(raw: &str, context: &str) {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    let gate = v.get("gate").expect("{context} MUST include gate (AC-007.80)");
    let host = v.get("host_watch").expect("{context} MUST include host_watch (AC-007.80)");
    let pool = v.get("pool").expect("{context} MUST include pool (AC-007.80)");
    let status = v.get("status").expect("{context} MUST include status (AC-007.80)");
    assert_pool_object(pool, context);
    assert_status_object(status, context);
    assert!(gate.get("gate_decision").is_some(), "gate MUST include gate_decision (AC-007.80)");
    assert!(host.get("load_1m").is_some(), "host_watch MUST include load_1m (AC-007.80)");

    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    let pool_pos = raw.find("\"pool\"").expect("pool key in raw JSON (AC-007.80)");
    let status_pos = raw.find("\"status\"").expect("status key in raw JSON (AC-007.80)");
    assert!(
        gate_pos < host_pos && host_pos < pool_pos && pool_pos < status_pos,
        "{context} MUST serialize gate → host_watch → pool → status (AC-007.80); got: {raw}"
    );
}

fn wrap_health_update(health_json: &str) -> String {
    format!(r#"{{"type":"health_update","health":{health_json}}}"#)
}

/// FR-007 / AC-007.80 — WS client decodes expanded HealthSnapshot pool + status siblings.
#[test]
fn fr007_ws_client_health_update_pool_status_decode() {
    use sharecli_ipc::ws_client::ClientMessage;

    let health = format!(
        r#"{{"managed_processes":3,"used_memory_mb":2048,"total_memory_mb":16384,
        "healthy":true,"gate":{{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"}},
        "host_watch":{{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}},{SAMPLE_POOL_TAIL},{SAMPLE_STATUS_TAIL}}}"#
    );
    let raw = wrap_health_update(&health);
    let msg = ClientMessage::from_json(&raw);
    match msg {
        ClientMessage::HealthUpdate(h) => {
            assert_eq!(h.managed_processes, 3);
            assert_eq!(h.pool.node_total, 2);
            assert_eq!(h.status.agents[0].pid, 99);
            assert_eq!(h.status.scanned, 50);
            assert_health_gate_host_watch_pool_status_order(&health, "HealthUpdate wire");
        }
        other => panic!("expected HealthUpdate, got {other:?}"),
    }
}

/// FR-007 / AC-007.80 — live IPC `health.status` payload wrapped as WS `health_update` decodes.
#[tokio::test]
async fn fr007_ws_client_health_update_live_ipc_wrap() {
    use sharecli_ipc::ws_client::ClientMessage;

    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");
    let resp = handler.dispatch(r#"{"id":1,"method":"health.status","params":{}}"#).await;
    assert!(resp.error.is_none(), "health.status MUST succeed (AC-007.80); err={:?}", resp.error);
    let health = serde_json::to_string(&resp.result).expect("serialize health.status result");
    assert_health_gate_host_watch_pool_status_order(&health, "health.status IPC");

    let raw = wrap_health_update(&health);
    let msg = ClientMessage::from_json(&raw);
    match msg {
        ClientMessage::HealthUpdate(h) => {
            assert!(h.pool.max_per_type > 0);
            // scanned/total_processes count known agents + managed pool procs
            // (not every host process), so they may legitimately be 0 on an
            // agent-less host; the wrap/decode contract is covered above.
            assert!(h.gate.gate_decision.len() > 0);
        }
        other => panic!(
            "health.status IPC wrapped as health_update MUST decode HealthUpdate (AC-007.80); got {other:?}"
        ),
    }
}

/// FR-007 / AC-007.80 — legacy health_update frames missing pool/status MUST NOT silently decode.
#[test]
fn fr007_ws_client_health_update_legacy_unknown() {
    use sharecli_ipc::ws_client::ClientMessage;

    let raw = r#"{"type":"health_update","health":{"managed_processes":3,"used_memory_mb":512,
        "total_memory_mb":16384,"healthy":true,
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}}"#;
    let msg = ClientMessage::from_json(raw);
    assert!(
        matches!(msg, ClientMessage::Unknown(_)),
        "legacy health_update without pool/status MUST yield Unknown (AC-007.80)"
    );
}
