//! FR: FR-003
//!
//! Phase 1 task 1.5 (lane 3 HIGH) — `config.set` is a blind write: the tray and
//! the CLI both patch the same config, so a concurrent write silently clobbers
//! the other. The audit's fix references RFC 9110 `If-Match`: expose a
//! `config.revision`, accept `if_revision` on `config.set`, and reject a stale
//! write with `CONFLICT` carrying the current revision.
//!
//! The revision is a content fingerprint rather than a counter, so it survives
//! a sidecar restart and needs no persisted state: `serde_json` is compiled
//! without `preserve_order`, so round-tripping through `Value` sorts object
//! keys and makes the serialization canonical even though `Config` stores its
//! maps as `HashMap`.

use serial_test::serial;
use sharecli_ipc::handler::{Handler, Response};
use sharecli_session::SessionStore;
use tempfile::TempDir;

/// Fields drop in declaration order: close the store before removing its
/// directory.
struct Fixture {
    handler: Handler,
    _directory: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("isolated handler directory");
        // `config.set` persists via `Config::save()`, which targets the
        // operator's live config path unless overridden.
        std::env::set_var("SHARECLI_CONFIG_PATH", directory.path().join("config.toml"));
        let store = SessionStore::open(directory.path().join("sessions.sqlite"))
            .expect("isolated handler store");
        Self { handler: Handler::with_session_store(store), _directory: directory }
    }

    async fn dispatch(&self, json: &str) -> Response {
        self.handler.dispatch(json).await
    }

    async fn revision(&self) -> String {
        let resp = self.dispatch(r#"{"id":1,"method":"config.revision","params":{}}"#).await;
        if let Some(error) = resp.error {
            panic!("config.revision failed: {error}");
        }
        resp.result["revision"].as_str().expect("revision must be a string").to_owned()
    }

    async fn set(&self, id: u64, key: &str, value: serde_json::Value, if_revision: Option<&str>) {
        let mut params = serde_json::json!({ "key": key, "value": value });
        if let Some(rev) = if_revision {
            params["if_revision"] = serde_json::json!(rev);
        }
        let req = serde_json::json!({ "id": id, "method": "config.set", "params": params });
        let resp = self.dispatch(&req.to_string()).await;
        assert!(resp.error.is_none(), "config.set {key} failed: {:?}", resp.error);
    }
}

/// `config.revision` must exist and be stable while the config is unchanged,
/// so a client can read it once and echo it back on its next write.
#[tokio::test]
#[serial]
async fn config_revision_is_exposed_and_stable_while_unchanged() {
    let fixture = Fixture::new();

    let first = fixture.revision().await;
    let second = fixture.revision().await;

    assert!(!first.is_empty(), "revision must not be empty");
    assert_eq!(first, second, "an unchanged config must keep its revision");
}

/// A write guarded by the revision the client actually holds must succeed.
#[tokio::test]
#[serial]
async fn config_set_with_the_current_if_revision_succeeds() {
    let fixture = Fixture::new();

    let current = fixture.revision().await;
    fixture.set(2, "runtime.max_processes", serde_json::json!(50), Some(&current)).await;
}

/// The core of the fix: a write carrying a revision that no longer matches must
/// be rejected with `CONFLICT`, and the error must name the *current* revision
/// so the client can refetch and retry instead of guessing.
///
/// The guard below is a literal, obviously stale string on purpose: the
/// assertion that must fail against the original code is "the write was
/// rejected at all", which does not depend on `config.revision` existing. The
/// original handler reads only `key` and `value`, so it accepts `if_revision`
/// silently — proven by a succeeding write rather than an error.
#[tokio::test]
#[serial]
async fn config_set_with_a_stale_if_revision_returns_conflict() {
    let fixture = Fixture::new();

    let resp = fixture
        .dispatch(
            r#"{"id":4,"method":"config.set","params":{
                 "key":"runtime.max_processes","value":80,"if_revision":"stale-revision"
               }}"#,
        )
        .await;

    let error = resp.error.expect("a stale if_revision must be rejected");
    assert!(error.contains("CONFLICT"), "stale write must report CONFLICT, got: {error}");
    assert!(
        error.contains(&fixture.revision().await),
        "CONFLICT must carry the current revision so the client can retry, got: {error}"
    );
}

/// A guard of the wrong JSON type must not be silently dropped. Treating
/// `if_revision: 42` as "no guard" would leave the write unguarded precisely
/// when the client believed it was guarded, so the failure is loud instead.
#[tokio::test]
#[serial]
async fn config_set_rejects_a_non_string_if_revision() {
    let fixture = Fixture::new();

    let resp = fixture
        .dispatch(
            r#"{"id":6,"method":"config.set","params":{
                 "key":"runtime.max_processes","value":80,"if_revision":42
               }}"#,
        )
        .await;

    let error = resp.error.expect("a non-string if_revision must be rejected");
    assert!(
        error.contains("if_revision must be a string"),
        "type error must name the offending field, got: {error}"
    );
}

/// The revision must actually track content: writing changes it, so two clients
/// cannot both believe they hold the latest one.
#[tokio::test]
#[serial]
async fn revision_changes_after_a_successful_set() {
    let fixture = Fixture::new();

    let before = fixture.revision().await;
    fixture.set(5, "runtime.max_memory_mb", serde_json::json!(8192), None).await;
    let after = fixture.revision().await;

    assert_ne!(before, after, "a successful write must advance the revision");
}
