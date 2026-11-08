// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — tray wire parity for `pool.status` + `status.snapshot` (AC-007.68)
//! FR: FR-007
//!
//! Tray/desktop consumers decode `PoolSnapshot` / `StatusSnapshot` from IPC without shelling
//! out to `sharecli pool --json` / `sharecli status --json` (parity with AC-007.47).

const HOST_WATCH_KEYS: [&str; 5] =
    ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

fn assert_json_gate_before_host_watch(raw: &str, context: &str) {
    let gate_pos = raw.find("\"gate\"").expect("gate key in raw JSON");
    let host_pos = raw.find("\"host_watch\"").expect("host_watch key in raw JSON");
    assert!(
        gate_pos < host_pos,
        "{context} MUST serialize gate before host_watch (AC-007.68); got: {raw}"
    );
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).expect("{context} MUST emit valid JSON");
    for key in HOST_WATCH_KEYS {
        assert!(
            v.get("host_watch").and_then(|h| h.get(key)).is_some(),
            "{context} host_watch MUST include {key} (AC-007.68)"
        );
    }
}

/// FR-007 / AC-007.68 — Linux tray wire shape decodes gate + host_watch from pool.status JSON.
#[test]
fn fr007_ipc_pool_status_tray_linux_wire_roundtrip() {
    use sharecli_tray_linux::ipc::PoolSnapshot;

    let raw = r#"{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: PoolSnapshot = serde_json::from_str(raw).expect("decode PoolSnapshot for tray");
    assert_eq!(snap.node_total, 2);
    assert!(snap.healthy);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "PoolSnapshot tray wire");
}

/// FR-007 / AC-007.68 — Linux tray wire shape decodes gate + host_watch from status.snapshot JSON.
#[test]
fn fr007_ipc_status_snapshot_tray_linux_wire_roundtrip() {
    use sharecli_tray_linux::ipc::StatusSnapshot;

    let raw = r#"{"total_processes":2,"agents":[{"pid":99,"family":"claude","comm":"claude",
        "state":"S","mem_rss_bytes":4096,"mem_rss":"4.0M","fd_count":12}],
        "scanned":50,"watched":1,
        "gate":{"thermal_pressure":"GREEN","detected_agents":1,
        "agent_total_rss_bytes":4096,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: StatusSnapshot = serde_json::from_str(raw).expect("decode StatusSnapshot for tray");
    assert_eq!(snap.total_processes, 2);
    assert_eq!(snap.agents[0].pid, 99);
    assert_eq!(snap.scanned, 50);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "StatusSnapshot tray wire");
}

/// FR-007 / AC-007.68 — Windows tray wire shape decodes gate + host_watch from pool.status JSON.
#[test]
fn fr007_ipc_pool_status_tray_windows_wire_roundtrip() {
    use sharecli_tray_windows::ipc::PoolSnapshot;

    let raw = r#"{"node_total":2,"node_idle":1,"bun_total":1,"bun_idle":0,"max_per_type":4,
        "healthy":true,"issues":[],
        "gate":{"thermal_pressure":"GREEN","detected_agents":0,
        "agent_total_rss_bytes":0,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: PoolSnapshot =
        serde_json::from_str(raw).expect("decode PoolSnapshot for Windows tray");
    assert_eq!(snap.node_total, 2);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_eq!(snap.host_watch.load_1m, 0.5);
    assert_json_gate_before_host_watch(raw, "PoolSnapshot Windows tray wire");
}

/// FR-007 / AC-007.68 — Windows tray wire shape decodes gate + host_watch from status.snapshot JSON.
#[test]
fn fr007_ipc_status_snapshot_tray_windows_wire_roundtrip() {
    use sharecli_tray_windows::ipc::StatusSnapshot;

    let raw = r#"{"total_processes":2,"agents":[{"pid":99,"family":"claude","comm":"claude",
        "state":"S","mem_rss_bytes":4096,"mem_rss":"4.0M","fd_count":12}],
        "scanned":50,"watched":1,
        "gate":{"thermal_pressure":"GREEN","detected_agents":1,
        "agent_total_rss_bytes":4096,"agent_contention":"OK","gate_decision":"ADMIT"},
        "host_watch":{"fd_count":1,"net_rx_bytes":2,"net_tx_bytes":3,
        "mem_rss_bytes":4,"load_1m":0.5}}"#;
    let snap: StatusSnapshot =
        serde_json::from_str(raw).expect("decode StatusSnapshot for Windows tray");
    assert_eq!(snap.agents[0].pid, 99);
    assert_eq!(snap.gate.gate_decision, "ADMIT");
    assert_json_gate_before_host_watch(raw, "StatusSnapshot Windows tray wire");
}

/// FR-007 / AC-007.68 — live IPC dispatch decodes through Linux tray wire types.
#[tokio::test]
async fn fr007_ipc_pool_status_tray_linux_live() {
    let handler = sharecli_ipc::handler::Handler::new().await.expect("Handler::new");

    let pool_resp = handler.dispatch(r#"{"id":1,"method":"pool.status","params":{}}"#).await;
    assert!(
        pool_resp.error.is_none(),
        "pool.status MUST succeed (AC-007.68); err={:?}",
        pool_resp.error
    );
    let pool_raw = serde_json::to_string(&pool_resp.result).expect("serialize pool.status result");
    let pool: sharecli_tray_linux::ipc::PoolSnapshot =
        serde_json::from_str(&pool_raw).expect("decode PoolSnapshot for tray");
    assert_json_gate_before_host_watch(&pool_raw, "pool.status tray live");
    assert!(!pool.gate.gate_decision.is_empty());

    let status_resp = handler.dispatch(r#"{"id":2,"method":"status.snapshot","params":{}}"#).await;
    assert!(
        status_resp.error.is_none(),
        "status.snapshot MUST succeed (AC-007.68); err={:?}",
        status_resp.error
    );
    let status_raw =
        serde_json::to_string(&status_resp.result).expect("serialize status.snapshot result");
    let status: sharecli_tray_linux::ipc::StatusSnapshot =
        serde_json::from_str(&status_raw).expect("decode StatusSnapshot for tray");
    assert_json_gate_before_host_watch(&status_raw, "status.snapshot tray live");
    assert!(!status.gate.gate_decision.is_empty());
}

/// FR-007 / AC-007.68 — Swift IPCClient exposes pool/status RPC + wire types.
#[test]
fn fr007_ipc_pool_status_tray_swift_wire_helpers() {
    let ipc_client = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift");
    assert!(
        ipc_client.contains("struct PoolSnapshot"),
        "IPCClient MUST define PoolSnapshot (AC-007.68)"
    );
    assert!(
        ipc_client.contains("struct StatusSnapshot"),
        "IPCClient MUST define StatusSnapshot (AC-007.68)"
    );
    assert!(
        ipc_client.contains("func poolStatus()"),
        "IPCClient MUST call pool.status (AC-007.68)"
    );
    assert!(
        ipc_client.contains("func statusSnapshot()"),
        "IPCClient MUST call status.snapshot (AC-007.68)"
    );
    assert!(
        ipc_client.contains("method: \"pool.status\""),
        "IPCClient poolStatus MUST use pool.status method (AC-007.68)"
    );
    assert!(
        ipc_client.contains("method: \"status.snapshot\""),
        "IPCClient statusSnapshot MUST use status.snapshot method (AC-007.68)"
    );
}

/// FR-007 / AC-007.68 — WinUI C# wire types decode pool/status IPC envelopes.
#[test]
fn fr007_ipc_pool_status_tray_csharp_wire_helpers() {
    let pool_cs = include_str!("../windows/ShareCLITray/PoolStatusSnapshot.cs");
    assert!(
        pool_cs.contains("class PoolSnapshot"),
        "PoolStatusSnapshot.cs MUST define PoolSnapshot (AC-007.68)"
    );
    assert!(
        pool_cs.contains("class StatusSnapshot"),
        "PoolStatusSnapshot.cs MUST define StatusSnapshot (AC-007.68)"
    );
    assert!(
        pool_cs.contains("TryParseIpcResponse"),
        "Pool/Status snapshots MUST decode IPC envelopes (AC-007.68)"
    );
    assert!(
        pool_cs.contains("class AgentProcRow"),
        "PoolStatusSnapshot.cs MUST define AgentProcRow (AC-007.68)"
    );
}
