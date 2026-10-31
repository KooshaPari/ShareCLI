// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Unit tests for `commands::serve` — loaded as `crate::commands::serve::tests`
//! via `#[path = "tests.rs"]` declared in `commands/serve/mod.rs`. The nested
//! `mod tests` block below creates `commands::serve::tests::tests`; `use super::*;`
//! pulls in the `crate::commands::serve::*` aliases from this file's outer scope.

use crate::commands::serve::*;
use crate::commands::{PoolJson, StatusJson};
use crate::monitoring::HostResourceWatchJson;
use crate::runtime::ProcState;
use axum::http::StatusCode;
use sharecli_fleet::GateStatusSnapshot;
use std::collections::HashMap;
use tokio::sync::broadcast;

#[cfg(test)]
mod tests {
    use sharecli_fleet::thermal::ThermalLevel;

    use super::*;
    use crate::serve_lock::{decide, Decision, OnConflict, ServeInfo, ServeState};

    // --- serve_lock decision tests ---

    fn running_live(url: &str) -> ServeState {
        ServeState::Running {
            info: ServeInfo {
                pid: std::process::id(),
                service: "sharecli".into(),
                url: url.into(),
                started_at_unix: 1,
            },
            stale: false,
        }
    }

    fn running_stale() -> ServeState {
        ServeState::Running {
            info: ServeInfo {
                pid: u32::MAX,
                service: "sharecli".into(),
                url: "http://127.0.0.1:9000".into(),
                started_at_unix: 1,
            },
            stale: true,
        }
    }

    #[test]
    fn free_state_always_serves() {
        assert_eq!(decide(&ServeState::Free, OnConflict::Abort), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Attach), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Replace), Decision::Serve);
        assert_eq!(decide(&ServeState::Free, OnConflict::Prompt), Decision::Serve);
    }

    #[test]
    fn stale_running_serves_regardless_of_policy() {
        let stale = running_stale();
        assert_eq!(decide(&stale, OnConflict::Abort), Decision::Serve);
        assert_eq!(decide(&stale, OnConflict::Attach), Decision::Serve);
    }

    #[test]
    fn live_running_abort_policy_aborts() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Abort), Decision::Abort);
        assert_eq!(decide(&live, OnConflict::Prompt), Decision::Abort);
    }

    #[test]
    fn live_running_attach_policy_attaches() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Attach), Decision::Attach);
    }

    #[test]
    fn live_running_replace_policy_replaces() {
        let live = running_live("http://127.0.0.1:9000");
        assert_eq!(decide(&live, OnConflict::Replace), Decision::Replace);
    }

    // --- parse_pressure_level unit tests ---

    #[test]
    fn parse_pressure_green() {
        assert_eq!(parse_pressure_level(1), Some(ThermalLevel::Green));
    }

    #[test]
    fn parse_pressure_yellow() {
        assert_eq!(parse_pressure_level(2), Some(ThermalLevel::Yellow));
    }

    #[test]
    fn parse_pressure_red() {
        assert_eq!(parse_pressure_level(4), Some(ThermalLevel::Red));
    }

    #[test]
    fn parse_pressure_unknown_returns_none() {
        assert_eq!(parse_pressure_level(0), None);
        assert_eq!(parse_pressure_level(3), None);
        assert_eq!(parse_pressure_level(5), None);
        assert_eq!(parse_pressure_level(255), None);
    }

    // --- ThermalEvent serialization tests ---

    #[test]
    fn thermal_event_warning_serializes() {
        let evt = ThermalEvent::ThermalWarning { pressure: 2 };
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"thermal_warning\""));
        assert!(s.contains("\"pressure\":2"));
    }

    #[test]
    fn thermal_event_critical_serializes() {
        let evt = ThermalEvent::ThermalCritical { pressure: 4 };
        let s = serde_json::to_string(&evt).unwrap();
        assert!(s.contains("\"event\":\"thermal_critical\""));
        assert!(s.contains("\"pressure\":4"));
    }

    // --- healthz / readyz JSON contract (no full server spin) ---

    #[test]
    fn healthz_json_is_ok() {
        let v = healthz_json();
        assert_eq!(v["status"], "ok");
    }

    /// Soft C08 live corpus: fixtures with `expect.health` must match `healthz_json`.
    #[test]
    fn corpus_health_fixtures_match_healthz() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/eval/corpus/scenarios");
        let mut checked = 0u32;
        for entry in std::fs::read_dir(&dir).expect("corpus scenarios dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read scenario");
            let data: serde_json::Value = serde_json::from_str(&raw).expect("parse scenario");
            let Some(health) = data.pointer("/expect/health").and_then(|v| v.as_str()) else {
                continue;
            };
            assert_eq!(
                healthz_json()["status"].as_str(),
                Some(health),
                "fixture {} expect.health mismatch",
                path.display()
            );
            checked += 1;
        }
        assert!(checked >= 1, "expected at least one corpus fixture with expect.health");
    }

    /// Soft C08 live corpus: fixtures with `expect.gate` must match thermal `gate_decision`.
    #[test]
    fn corpus_thermal_gate_fixtures_match_gate_decision() {
        use sharecli_thermal_tui::gate_decision;

        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/eval/corpus/scenarios");
        let mut checked = 0u32;
        for entry in std::fs::read_dir(&dir).expect("corpus scenarios dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read scenario");
            let data: serde_json::Value = serde_json::from_str(&raw).expect("parse scenario");
            let Some(expect_gate) = data.pointer("/expect/gate").and_then(|v| v.as_str()) else {
                continue;
            };
            let level = match data.get("thermal").and_then(|v| v.as_str()) {
                Some("red") => ThermalLevel::Red,
                Some("yellow") => ThermalLevel::Yellow,
                Some("green") | None => ThermalLevel::Green,
                Some(other) => panic!("fixture {}: unknown thermal {:?}", path.display(), other),
            };
            assert_eq!(
                gate_decision(level),
                expect_gate,
                "fixture {} expect.gate mismatch for {:?}",
                path.display(),
                level
            );
            if let Some(spawn_allowed) =
                data.pointer("/expect/spawn_allowed").and_then(|v| v.as_bool())
            {
                let allowed = expect_gate != "DENY";
                assert_eq!(
                    spawn_allowed,
                    allowed,
                    "fixture {} expect.spawn_allowed inconsistent with gate",
                    path.display()
                );
            }
            checked += 1;
        }
        assert!(checked >= 1, "expected at least one corpus fixture with expect.gate");
    }

    #[test]
    fn readyz_ok_while_serving() {
        let (status, body) = readyz_response(false);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
    }

    #[test]
    fn readyz_unavailable_after_shutdown() {
        let (status, body) = readyz_response(true);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unavailable");
    }

    // --- render_prometheus_metrics unit tests ---

    fn make_process(name: &str, memory_mb: u64) -> crate::runtime::ProcessInfo {
        crate::runtime::ProcessInfo {
            pid: 1234,
            name: name.to_string(),
            cmd: vec!["fake".to_string()],
            memory_mb,
            start_time: 0,
            cpu_percent: 0.0,
            project: None,
            harness: None,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        }
    }

    fn make_health(healthy: bool, failures: u32) -> crate::health_check::HealthStatus {
        crate::health_check::HealthStatus {
            healthy,
            consecutive_failures: failures,
            last_error: None,
            last_check: std::time::Instant::now(),
        }
    }

    #[test]
    fn prometheus_output_contains_required_metric_names() {
        let processes = vec![make_process("myapp", 256)];
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("myapp".to_string(), make_health(true, 0));
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(out.contains("sharecli_process_memory_mb"), "missing memory metric");
        assert!(out.contains("sharecli_process_up"), "missing process_up metric");
        assert!(
            out.contains("sharecli_health_check_consecutive_failures"),
            "missing failures metric"
        );
        assert!(out.contains("sharecli_health_check_status"), "missing health_check_status metric");
    }

    #[test]
    fn prometheus_process_up_reflects_health_status() {
        let processes = vec![make_process("svc", 64)];

        let mut healthy_map = std::collections::HashMap::new();
        healthy_map.insert("svc".to_string(), make_health(true, 0));
        let healthy_out = render_prometheus_metrics(&processes, &healthy_map);
        assert!(
            healthy_out.contains("sharecli_process_up{process=\"svc\"} 1"),
            "healthy process should have process_up=1"
        );

        let mut unhealthy_map = std::collections::HashMap::new();
        unhealthy_map.insert("svc".to_string(), make_health(false, 3));
        let unhealthy_out = render_prometheus_metrics(&processes, &unhealthy_map);
        assert!(
            unhealthy_out.contains("sharecli_process_up{process=\"svc\"} 0"),
            "unhealthy process should have process_up=0"
        );
    }

    #[test]
    fn prometheus_gauge_values_match_inputs() {
        let processes = vec![make_process("worker", 512)];
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("worker".to_string(), make_health(true, 7));
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(out.contains("sharecli_process_memory_mb{process=\"worker\"} 512"));
        assert!(out.contains("sharecli_health_check_consecutive_failures{process=\"worker\"} 7"));
    }

    #[test]
    fn prometheus_label_escaping_handles_special_chars() {
        // Process name with double-quote and backslash
        let processes = vec![make_process("my\"app\\test", 128)];
        let hmap = std::collections::HashMap::new();
        let out = render_prometheus_metrics(&processes, &hmap);
        // Escaped label value should appear; raw chars must not appear unescaped inside quotes
        assert!(
            out.contains(r#"process="my\"app\\test""#),
            "label value not properly escaped: {out}"
        );
    }

    #[test]
    fn prometheus_process_up_defaults_to_1_when_no_health_check() {
        // Process present in pool but NOT in health_store → should be up=1
        let processes = vec![make_process("orphan", 32)];
        let hmap = std::collections::HashMap::new(); // empty
        let out = render_prometheus_metrics(&processes, &hmap);
        assert!(
            out.contains("sharecli_process_up{process=\"orphan\"} 1"),
            "process without health check should default to up=1"
        );
    }

    // --- broadcast channel test ---

    #[tokio::test]
    async fn thermal_broadcast_delivers_to_subscriber() {
        let (tx, mut rx) = broadcast::channel::<ThermalEvent>(8);
        tx.send(ThermalEvent::ThermalWarning { pressure: 2 }).unwrap();
        let received = rx.recv().await.unwrap();
        matches!(received, ThermalEvent::ThermalWarning { pressure: 2 });
    }

    const HOST_WATCH_KEYS: [&str; 5] =
        ["fd_count", "net_rx_bytes", "net_tx_bytes", "mem_rss_bytes", "load_1m"];

    fn ensure_test_config() {
        let _ = crate::config::init_global();
    }

    /// FR-007 / AC-007.41 — dashboard WS snapshot carries gate + host_watch operator envelope.
    #[tokio::test]
    async fn dashboard_ws_snapshot_includes_gate_and_host_watch() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

        assert!(
            v.get("gate").and_then(|g| g.get("gate_decision")).is_some(),
            "dashboard WS MUST include gate (AC-007.41); got: {v}"
        );
        let host = v.get("host_watch").expect("dashboard WS MUST include host_watch (AC-007.41)");
        for key in HOST_WATCH_KEYS {
            assert!(host.get(key).is_some(), "host_watch MUST include {key} (AC-007.41)");
        }
        assert!(v.get("agents").is_some(), "dashboard WS MUST include agents summary (AC-007.41)");
        assert!(v.get("processes").and_then(|p| p.as_array()).is_some());
    }

    /// FR-007 / AC-007.70 — dashboard WS snapshot carries pool + status IPC siblings.
    #[tokio::test]
    async fn dashboard_ws_snapshot_includes_pool_and_status() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

        let pool = v.get("pool").expect("dashboard WS MUST include pool (AC-007.70)");
        assert!(pool.get("node_total").is_some(), "pool MUST include node_total (AC-007.70)");
        assert!(pool.get("healthy").is_some(), "pool MUST include healthy (AC-007.70)");
        let status = v.get("status").expect("dashboard WS MUST include status (AC-007.70)");
        assert!(
            status.get("total_processes").is_some(),
            "status MUST include total_processes (AC-007.70)"
        );
        assert!(
            status.get("scanned").is_some() && status.get("watched").is_some(),
            "status MUST include scanned/watched (AC-007.70)"
        );
    }

    /// FR-007 / AC-007.41 — dashboard WS snapshot preserves gate → host_watch key ordering.
    #[tokio::test]
    async fn dashboard_ws_snapshot_gate_before_host_watch() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let gate_pos = raw.find("\"gate\"").expect("gate key in dashboard WS JSON");
        let host_pos = raw.find("\"host_watch\"").expect("host_watch key in dashboard WS JSON");
        assert!(
            gate_pos < host_pos,
            "dashboard WS MUST serialize gate before host_watch (AC-007.41); got: {raw}"
        );
    }

    /// FR-007 / AC-007.70 — dashboard WS snapshot preserves gate → host_watch → pool → status → agents order.
    #[tokio::test]
    async fn dashboard_ws_snapshot_operator_key_order() {
        ensure_test_config();
        let snap = build_dashboard_ws_snapshot().await.expect("dashboard snapshot");
        let raw = serde_json::to_string(&snap).expect("serialize dashboard snapshot");
        let gate_pos = raw.find("\"gate\"").expect("gate key");
        let host_pos = raw.find("\"host_watch\"").expect("host_watch key");
        let pool_pos = raw.find("\"pool\"").expect("pool key (AC-007.70)");
        let status_pos = raw.find("\"status\"").expect("status key (AC-007.70)");
        let agents_pos = raw.find("\"agents\"").expect("agents key");
        let processes_pos = raw.find("\"processes\"").expect("processes key");
        assert!(
            gate_pos < host_pos
                && host_pos < pool_pos
                && pool_pos < status_pos
                && status_pos < agents_pos
                && agents_pos < processes_pos,
            "dashboard WS MUST serialize gate → host_watch → pool → status → agents → processes (AC-007.70); got: {raw}"
        );
    }

    // --- escape_label_value edge cases ---

    #[test]
    fn escape_label_value_empty_string() {
        assert_eq!(escape_label_value(""), "");
    }

    #[test]
    fn escape_label_value_no_special_chars() {
        assert_eq!(escape_label_value("hello world"), "hello world");
    }

    #[test]
    fn escape_label_value_newline_escaped() {
        assert_eq!(escape_label_value("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn escape_label_value_all_special_chars() {
        // Input has real backslash, double-quote, and newline chars
        let input = "back\\slash and \"quotes\" and newline\n";
        let escaped = escape_label_value(input);
        // backslash \ becomes \\
        assert!(escaped.contains("\\\\"), "backslash must be escaped to double-backslash");
        // double-quote " becomes \"
        assert!(escaped.contains("\\\""), "quotes must be escaped");
        // newline becomes \n (the two literal chars backslash + n)
        assert!(escaped.contains("\\n"), "newline must be escaped");
    }

    // --- render_prometheus_metrics edge cases ---

    #[test]
    fn prometheus_empty_processes_empty_health() {
        let out = render_prometheus_metrics(&[], &std::collections::HashMap::new());
        assert!(out.contains("sharecli_process_memory_mb"), "must have memory metric header");
        assert!(out.contains("sharecli_process_up"), "must have process_up metric header");
        assert!(out.contains("sharecli_health_check_consecutive_failures"));
        assert!(out.contains("sharecli_health_check_status"));
    }

    #[test]
    fn prometheus_health_only_no_processes() {
        let mut hmap = std::collections::HashMap::new();
        hmap.insert("ghost-svc".to_string(), make_health(true, 0));
        let out = render_prometheus_metrics(&[], &hmap);
        // health-only entry should appear in failures and status metrics
        assert!(out.contains("sharecli_health_check_consecutive_failures{process=\"ghost-svc\"} 0"));
        assert!(out.contains("sharecli_health_check_status{process=\"ghost-svc\"} 1"));
    }

    // --- DashboardProcessRow serialization ---

    #[test]
    fn dashboard_process_row_serialization() {
        let row = DashboardProcessRow {
            pid: 42,
            name: "claude".into(),
            cmd: vec!["claude".into(), "serve".into()],
            memory_mb: 256,
            project: Some("myproject".into()),
            harness: Some("jcode".into()),
            start_time: 1_700_000_000,
        };
        let json = serde_json::to_string(&row).expect("serialize");
        assert!(json.contains("\"pid\":42"));
        assert!(json.contains("\"name\":\"claude\""));
        assert!(json.contains("\"memory_mb\":256"));
        assert!(json.contains("\"project\":\"myproject\""));
        assert!(json.contains("\"harness\":\"jcode\""));
    }

    #[test]
    fn dashboard_process_row_none_optional_fields_skipped() {
        let row = DashboardProcessRow {
            pid: 1,
            name: "x".into(),
            cmd: vec![],
            memory_mb: 0,
            project: None,
            harness: None,
            start_time: 0,
        };
        let json = serde_json::to_string(&row).expect("serialize");
        assert!(!json.contains("\"project\""), "None project must be skipped");
        assert!(!json.contains("\"harness\""), "None harness must be skipped");
    }

    // --- DashboardAgentSummary serialization ---

    #[test]
    fn dashboard_agent_summary_serialization() {
        let summary = DashboardAgentSummary {
            scanned: 10,
            watched: 5,
            total_rss_bytes: 1_000_000,
            families: HashMap::from([("claude".into(), 3), ("cursor".into(), 2)]),
        };
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(json.contains("\"scanned\":10"));
        assert!(json.contains("\"watched\":5"));
        assert!(json.contains("\"total_rss_bytes\":1000000"));
        assert!(json.contains("\"claude\""));
        assert!(json.contains("\"cursor\""));
    }

    // --- DashboardWsSnapshot serialization ---

    #[test]
    fn dashboard_ws_snapshot_serializes_all_fields() {
        let gate_snap = GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 0,
            agent_total_rss_bytes: 0,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let hw = HostResourceWatchJson::default();
        let snapshot = DashboardWsSnapshot {
            gate: gate_snap.clone(),
            host_watch: hw,
            pool: PoolJson {
                node_total: 0,
                node_idle: 0,
                bun_total: 0,
                bun_idle: 0,
                max_per_type: 0,
                healthy: true,
                issues: vec![],
                gate: gate_snap.clone(),
                host_watch: hw,
                status: None,
            },
            status: StatusJson {
                total_processes: 0,
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: gate_snap,
                host_watch: hw,
                pool: None,
                log_location: None,
            },
            agents: DashboardAgentSummary {
                scanned: 0,
                watched: 0,
                total_rss_bytes: 0,
                families: HashMap::new(),
            },
            processes: vec![],
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"gate\""));
        assert!(json.contains("\"host_watch\""));
        assert!(json.contains("\"pool\""));
        assert!(json.contains("\"status\""));
        assert!(json.contains("\"agents\""));
        assert!(json.contains("\"processes\""));
    }
}
