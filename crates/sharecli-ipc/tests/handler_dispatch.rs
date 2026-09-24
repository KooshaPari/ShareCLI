//! FR: FR-003

//! FR-003 / C01 — sharecli-ipc JSON-RPC handler dispatch coverage.

use sharecli_ipc::handler::Handler;
use sharecli_session::SessionStore;
use tempfile::TempDir;

/// Fields drop in declaration order: close the store before removing its directory.
struct HandlerFixture {
    handler: Handler,
    _directory: TempDir,
}

impl HandlerFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("isolated handler directory");
        // config.set persists via Config::save(), which writes to the
        // operator's live config path unless overridden. Redirect it into this
        // fixture's temp directory so running the suite can never store a
        // default-derived config over a real one. The dispatch assertions read
        // the handler's in-memory config, so they are unaffected.
        std::env::set_var("SHARECLI_CONFIG_PATH", directory.path().join("config.toml"));
        let store = SessionStore::open(directory.path().join("sessions.sqlite"))
            .expect("isolated handler store");
        Self { handler: Handler::with_session_store(store), _directory: directory }
    }
}

#[tokio::test]
async fn fr003_ipc_handler_process_list_and_health() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let list_resp = handler.dispatch(r#"{"id":1,"method":"process.list","params":{}}"#).await;
    assert!(list_resp.error.is_none(), "process.list error: {:?}", list_resp.error);
    assert!(list_resp.result.is_array());

    let health_resp = handler.dispatch(r#"{"id":2,"method":"health.status","params":{}}"#).await;
    assert!(health_resp.error.is_none(), "health.status error: {:?}", health_resp.error);
    assert!(health_resp.result.get("healthy").is_some());
}

#[tokio::test]
async fn fr003_ipc_handler_config_get_and_unknown_method() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let cfg_resp = handler.dispatch(r#"{"id":3,"method":"config.get","params":{}}"#).await;
    assert!(cfg_resp.error.is_none());
    assert!(cfg_resp.result.get("projects").is_some());

    let bad_resp = handler.dispatch(r#"{"id":4,"method":"nope.method","params":{}}"#).await;
    assert!(bad_resp.error.is_some());
    assert!(bad_resp.error.unwrap().contains("unknown method"));
}

#[tokio::test]
async fn fr003_ipc_handler_rejects_invalid_json() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let resp = handler.dispatch("not-json").await;
    assert!(resp.error.is_some());
    assert!(resp.error.unwrap().contains("parse error"));
}

#[tokio::test]
async fn fr003_ipc_handler_config_set_and_process_kill_all() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let set_resp = handler
        .dispatch(
            r#"{"id":6,"method":"config.set","params":{"key":"pool.max_per_type","value":3}}"#,
        )
        .await;
    assert!(set_resp.error.is_none(), "config.set error: {:?}", set_resp.error);
    assert_eq!(set_resp.result, serde_json::json!(true));

    let get_resp = handler.dispatch(r#"{"id":7,"method":"config.get","params":{}}"#).await;
    assert!(get_resp.error.is_none());
    assert_eq!(get_resp.result["pool"]["max_per_type"], 3);

    let kill_all = handler.dispatch(r#"{"id":8,"method":"process.kill_all","params":{}}"#).await;
    assert!(kill_all.error.is_none(), "process.kill_all error: {:?}", kill_all.error);
    assert_eq!(kill_all.result, serde_json::json!(true));
}

#[tokio::test]
async fn fr003_ipc_handler_process_kill_missing_pid() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let resp = handler.dispatch(r#"{"id":9,"method":"process.kill","params":{}}"#).await;
    assert!(resp.error.is_some());
}

#[tokio::test]
async fn fr003_ipc_handler_monitoring_report() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let resp = handler.dispatch(r#"{"id":10,"method":"monitoring.report","params":{}}"#).await;
    assert!(resp.error.is_none(), "monitoring.report error: {:?}", resp.error);
    assert!(resp.result.is_object());
}

/// FR-003 / C01 — pool.status + status.snapshot nest gate/host_watch siblings.
#[tokio::test]
async fn fr003_ipc_handler_pool_status_and_status_snapshot() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let pool_resp = handler.dispatch(r#"{"id":11,"method":"pool.status","params":{}}"#).await;
    assert!(pool_resp.error.is_none(), "pool.status error: {:?}", pool_resp.error);
    assert!(pool_resp.result.get("max_per_type").is_some());
    assert!(pool_resp.result.get("gate").is_some());
    assert!(pool_resp.result.get("host_watch").is_some());
    assert!(pool_resp.result.get("status").is_some());

    let status_resp = handler.dispatch(r#"{"id":12,"method":"status.snapshot","params":{}}"#).await;
    assert!(status_resp.error.is_none(), "status.snapshot error: {:?}", status_resp.error);
    assert!(status_resp.result.get("total_processes").is_some());
    assert!(status_resp.result.get("agents").is_some());
    assert!(status_resp.result.get("pool").is_some());
    assert!(status_resp.result.get("gate").is_some());
}

/// FR-003 / C01 — config.set rejects missing key; process.kill rejects bad pid type.
#[tokio::test]
async fn fr003_ipc_handler_config_set_missing_key_and_kill_bad_pid() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let missing_key =
        handler.dispatch(r#"{"id":13,"method":"config.set","params":{"value":1}}"#).await;
    assert!(missing_key.error.is_some());
    assert!(missing_key.error.unwrap().contains("missing key"));

    let bad_pid =
        handler.dispatch(r#"{"id":14,"method":"process.kill","params":{"pid":"nope"}}"#).await;
    assert!(bad_pid.error.is_some());
}

#[tokio::test]
async fn fr003_ipc_handler_session_recovery_methods_are_dry_run_safe() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let list = handler.dispatch(r#"{"id":15,"method":"session.list","params":{}}"#).await;
    assert!(list.error.is_none(), "session.list error: {:?}", list.error);
    assert!(list.result.is_array());

    let plan = handler.dispatch(r#"{"id":16,"method":"recovery.plan","params":{}}"#).await;
    assert!(plan.error.is_none(), "recovery.plan error: {:?}", plan.error);
    assert!(plan.result.is_array());

    let execute = handler
        .dispatch(r#"{"id":17,"method":"recovery.execute","params":{"execute":false}}"#)
        .await;
    assert!(execute.error.is_none(), "recovery.execute error: {:?}", execute.error);
    assert!(execute.result.is_array());

    let invalid_age = handler
        .dispatch(r#"{"id":171,"method":"recovery.plan","params":{"max_age_seconds":0}}"#)
        .await;
    assert!(invalid_age.error.is_some(), "zero max_age_seconds must be rejected");

    let observe = handler
        .dispatch(
            &serde_json::json!({
                "id": 18,
                "method": "session.observe",
                "params": {
                    "observed_at": "2026-07-31T08:00:00Z",
                    "surface": {
                        "id": "test:ipc",
                        "terminal": "ghostty",
                        "title": null,
                        "cwd": "/tmp",
                        "process": null
                    },
                    "session": null,
                    "capabilities": {
                        "read": false,
                        "write": false,
                        "resize": false,
                        "layout": false,
                        "durable_pty": false
                    },
                    "kind": "Updated"
                }
            })
            .to_string(),
        )
        .await;
    assert!(observe.error.is_none(), "session.observe error: {:?}", observe.error);

    let observations = handler
        .dispatch(r#"{"id":19,"method":"session.observations","params":{"surface_id":"test:ipc"}}"#)
        .await;
    assert!(observations.error.is_none(), "session.observations error: {:?}", observations.error);
    assert!(observations.result.as_array().is_some_and(|rows| !rows.is_empty()));

    let compact = handler.dispatch(r#"{"id":20,"method":"session.compact","params":{}}"#).await;
    assert!(compact.error.is_none(), "session.compact error: {:?}", compact.error);
    assert!(compact.result.get("removed").is_some());
}

#[tokio::test]
async fn fr003_ipc_handler_persists_and_lists_validated_layouts() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let snapshot = serde_json::json!({
        "id": "ipc-layout",
        "terminal": "ghostty",
        "captured_at": "2026-08-01T00:00:00Z",
        "root": {
            "Split": {
                "axis": "Horizontal",
                "ratio_millis": 500,
                "children": [
                    {"Pane": {"surface_id": "ghostty:1"}},
                    {"Pane": {"surface_id": "ghostty:2"}}
                ]
            }
        }
    });

    let save = handler
        .dispatch(
            &serde_json::json!({
                "id": 21,
                "method": "layout.save",
                "params": {"snapshot": snapshot}
            })
            .to_string(),
        )
        .await;
    assert!(save.error.is_none(), "layout.save error: {:?}", save.error);
    assert_eq!(save.result["id"], "ipc-layout");

    let list = handler.dispatch(r#"{"id":22,"method":"layout.list","params":{}}"#).await;
    assert!(list.error.is_none(), "layout.list error: {:?}", list.error);
    assert!(list
        .result
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["id"] == "ipc-layout")));

    let inspect = handler
        .dispatch(r#"{"id":23,"method":"layout.inspect","params":{"id":"ipc-layout"}}"#)
        .await;
    assert!(inspect.error.is_none(), "layout.inspect error: {:?}", inspect.error);
    assert_eq!(inspect.result["id"], "ipc-layout");
}

/// The tray polls `pool.effectiveness` on a five-second cadence. Its decoder
/// requires both meter blocks plus a Unix sample timestamp.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fr003_ipc_handler_pool_effectiveness_returns_typed_snapshot() {
    let fixture = HandlerFixture::new();
    let response = fixture
        .handler
        .dispatch(r#"{"id":24,"method":"pool.effectiveness","params":{}}"#)
        .await;

    assert!(response.error.is_none(), "pool.effectiveness error: {:?}", response.error);
    for field in ["hits", "misses", "nocache_runs"] {
        assert!(response.result["coalesce"][field].is_u64(), "missing coalesce.{field}");
    }
    for field in ["acquires", "waits", "timeouts"] {
        assert!(response.result["slot_queue"][field].is_u64(), "missing slot_queue.{field}");
    }
    assert!(response.result["sampled_at"].as_u64().is_some_and(|value| value > 0));
}

/// A successful spawn must return a server-assigned PID, retain the child in
/// `process.list`, and stop it through the same handler-owned pool.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fr003_ipc_handler_process_spawn_retains_and_kills_child() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;
    let spawned = handler
        .dispatch(
            &serde_json::json!({
                "id": 25,
                "method": "process.spawn",
                "params": {
                    "name": "/bin/sleep",
                    "command": "/bin/sleep",
                    "args": ["5"],
                    "project": "ipc-dispatch-test",
                    "harness": null
                }
            })
            .to_string(),
        )
        .await;

    assert!(spawned.error.is_none(), "process.spawn error: {:?}", spawned.error);
    assert_eq!(spawned.result["success"], serde_json::json!(true));
    let pid = spawned.result["pid"].as_u64().expect("server-assigned pid") as u32;
    assert!(pid > 0);

    let listed = handler.dispatch(r#"{"id":26,"method":"process.list","params":{}}"#).await;
    assert!(listed.error.is_none(), "process.list error: {:?}", listed.error);
    assert!(listed
        .result
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["pid"] == pid)));

    let killed = handler
        .dispatch(&serde_json::json!({"id": 27, "method": "process.kill", "params": {"pid": pid}}).to_string())
        .await;
    assert!(killed.error.is_none(), "process.kill error: {:?}", killed.error);
    assert_eq!(killed.result, serde_json::json!(true));
}

/// Malformed spawn requests fail loudly in the envelope; an unrunnable
/// command returns the typed `{pid, success, error}` failure the Swift
/// decoder expects. Empty `args` is valid — it is the Spawn form default.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fr003_ipc_handler_process_spawn_validates_and_reports_failure() {
    let fixture = HandlerFixture::new();
    let handler = &fixture.handler;

    let missing = handler
        .dispatch(r#"{"id":28,"method":"process.spawn","params":{"args":[]}}"#)
        .await;
    assert!(
        missing.error.as_deref().is_some_and(|e| e.contains("command")),
        "missing command must be rejected: {:?}",
        missing.error
    );

    let blank = handler
        .dispatch(r#"{"id":29,"method":"process.spawn","params":{"command":"   ","args":[]}}"#)
        .await;
    assert!(
        blank.error.as_deref().is_some_and(|e| e.contains("command")),
        "blank command must be rejected: {:?}",
        blank.error
    );

    let bad_args = handler
        .dispatch(r#"{"id":30,"method":"process.spawn","params":{"command":"/bin/echo","args":"x"}}"#)
        .await;
    assert!(
        bad_args.error.as_deref().is_some_and(|e| e.contains("args")),
        "non-array args must be rejected: {:?}",
        bad_args.error
    );

    let failed = handler
        .dispatch(
            r#"{"id":31,"method":"process.spawn","params":{"command":"/nonexistent/sharecli-probe","args":[]}}"#,
        )
        .await;
    assert!(
        failed.error.is_none(),
        "runtime spawn failure must be a typed result, not an envelope error: {:?}",
        failed.error
    );
    assert_eq!(failed.result["success"], serde_json::json!(false));
    assert_eq!(failed.result["pid"], serde_json::json!(0));
    assert!(failed.result["error"].as_str().is_some_and(|e| !e.is_empty()));
}
