// SPDX-License-Identifier: Apache-2.0 OR MIT
//! E2E integration tests for the teamcomm daemon wire protocol.
//!
//! Exercises all 14 wire methods via direct handler invocation (same
//! pattern as `integration.rs`). Each test creates a fresh `AppState`
//! and validates end-to-end flows: session lifecycle, cross-session
//! inbox, reservation claim/release, state-set/get, discovery filtering.
//!
//! # Wire contract (verified against `crates/teamcomm-daemon/src/handlers.rs`)
//!
//! ## Methods + required payload fields
//!
//! | Method | Required payload | Response shape |
//! |--------|------------------|----------------|
//! | `session.register` | `pid`, `agent_type`, `working_dir`, `capabilities` | full `Session` |
//! | `session.deregister` | `session_id` | (success) |
//! | `session.heartbeat` | `session_id` | (success) |
//! | `session.list` | (none, filters: `agent_type`, `status`) | `Vec<Session>` |
//! | `session.get` | `session_id` | full `Session` |
//! | `reservation.claim` | `session_id`, `path`, `mode`, `ttl_sec` | `{reservation_id, conflicts}` |
//! | `reservation.release` | `session_id`, `reservation_id` | (success) |
//! | `reservation.list` | `path` filter | `Vec<Reservation>` |
//! | `inbox.post` | `from_session`, `to_session`, `body`, `kind`, `priority` | `{message_id}` |
//! | `inbox.list` | `session_id` (recipient) | `Vec<InboxMessage>` |
//! | `inbox.read` | `session_id`, `message_id` | full `InboxMessage` |
//! | `state.set` | `session_id`, `status`, `focus_file?`, `focus_branch?`, `worktree?` | `{ok: true}` |
//! | `state.get` | `session_id` | full `LiveState` |
//! | `discover.agents` | `path?`, `branch?`, `capabilities?` | `Vec<SessionSummary>` |
//!
//! ## Status enum (parse_agent_status accepts):
//!
//! `idle`, `working`, `blocked`, `done`, `paused`
//!
//! ## Reservation modes (parse_reservation_mode accepts):
//!
//! `read`, `write`, `exclusive`
//!
//! ## Priorities (parse_priority accepts):
//!
//! `low`, `normal`, `high`, `urgent`

use serde_json::{json, Value};
use teamcomm_daemon::handlers;
use teamcomm_daemon::state::{self, AppState};

fn fresh_state() -> AppState {
    state::new_state()
}

async fn call(state: &AppState, method: &str, payload: Value) -> Result<Value, String> {
    let result = match method {
        "session.register" => {
            handlers::handle_session_register(state.clone(), payload).await
        }
        "session.deregister" => {
            handlers::handle_session_deregister(state.clone(), payload).await
        }
        "session.heartbeat" => {
            handlers::handle_session_heartbeat(state.clone(), payload).await
        }
        "session.list" => handlers::handle_session_list(state.clone(), payload).await,
        "session.get" => handlers::handle_session_get(state.clone(), payload).await,
        "reservation.claim" => {
            handlers::handle_reservation_claim(state.clone(), payload).await
        }
        "reservation.release" => {
            handlers::handle_reservation_release(state.clone(), payload).await
        }
        "reservation.list" => {
            handlers::handle_reservation_list(state.clone(), payload).await
        }
        "inbox.post" => handlers::handle_inbox_post(state.clone(), payload).await,
        "inbox.list" => handlers::handle_inbox_list(state.clone(), payload).await,
        "inbox.read" => handlers::handle_inbox_read(state.clone(), payload).await,
        "state.set" => handlers::handle_state_set(state.clone(), payload).await,
        "state.get" => handlers::handle_state_get(state.clone(), payload).await,
        "discover.agents" => {
            handlers::handle_discover_agents(state.clone(), payload).await
        }
        other => panic!("unknown method: {}", other),
    };
    result.map_err(|e| format!("{:?}", e))
}

fn assert_ok(r: Result<Value, String>, ctx: &str) -> Value {
    match r {
        Ok(v) => v,
        Err(e) => panic!("[{}] expected ok, got error: {}", ctx, e),
    }
}

/// Register a session and return its `session_id`. Each agent gets a unique
/// PID via a per-call counter to avoid re-registration aliasing (the daemon
/// deduplicates by pid).
async fn register_session(state: &AppState, agent_type: &str) -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32 + 50000;
    let v = assert_ok(
        call(
            state,
            "session.register",
            json!({
                "agent_type": agent_type,
                "pid": pid,
                "working_dir": "/tmp/teamcomm-test",
                "capabilities": ["rust", "git:write"],
            }),
        )
        .await,
        "session.register",
    );
    v["session_id"]
        .as_str()
        .expect("session_id")
        .to_string()
}

// ─────────────────────────── tests ───────────────────────────

#[tokio::test]
async fn register_heartbeat_deregister_lifecycle() {
    let state = fresh_state();
    let sid = register_session(&state, "codex").await;

    assert_ok(
        call(&state, "session.heartbeat", json!({ "session_id": sid })).await,
        "session.heartbeat",
    );
    assert_ok(
        call(&state, "session.deregister", json!({ "session_id": sid })).await,
        "session.deregister",
    );
}

#[tokio::test]
async fn session_list_returns_registered_sessions() {
    let state = fresh_state();
    let sid_a = register_session(&state, "codex").await;
    let sid_b = register_session(&state, "claude").await;

    // session.list returns Vec<Session> directly (not wrapped)
    let v = assert_ok(call(&state, "session.list", json!({})).await, "session.list");
    let sessions = v.as_array().expect("response is array");
    let ids: Vec<&str> = sessions
        .iter()
        .filter_map(|s| s["session_id"].as_str())
        .collect();
    assert!(ids.contains(&sid_a.as_str()));
    assert!(ids.contains(&sid_b.as_str()));
}

#[tokio::test]
async fn session_get_returns_full_session_record() {
    let state = fresh_state();
    let sid = register_session(&state, "codex").await;

    let v = assert_ok(
        call(&state, "session.get", json!({ "session_id": sid })).await,
        "session.get",
    );
    assert_eq!(v["session_id"], sid);
    assert_eq!(v["agent_type"], "codex");
}

#[tokio::test]
async fn reservation_claim_release_round_trip() {
    let state = fresh_state();
    let sid = register_session(&state, "codex").await;

    // ttl_sec (not ttl_seconds); mode defaults to "write" if absent
    let claim = assert_ok(
        call(
            &state,
            "reservation.claim",
            json!({
                "session_id": sid,
                "path": "src/auth/login.rs",
                "mode": "exclusive",
                "ttl_sec": 60,
            }),
        )
        .await,
        "reservation.claim",
    );
    let rid = claim["reservation_id"]
        .as_str()
        .expect("reservation_id")
        .to_string();

    // reservation.list filters by path (not session_id)
    let list = assert_ok(
        call(
            &state,
            "reservation.list",
            json!({ "path": "src/auth/login.rs" }),
        )
        .await,
        "reservation.list",
    );
    let reservations = list.as_array().expect("response is array");
    let rids: Vec<&str> = reservations
        .iter()
        .filter_map(|r| r["reservation_id"].as_str())
        .collect();
    assert!(
        rids.contains(&rid.as_str()),
        "reservation_id {} not in list {:?}",
        rid,
        rids
    );

    assert_ok(
        call(
            &state,
            "reservation.release",
            json!({ "session_id": sid, "reservation_id": rid }),
        )
        .await,
        "reservation.release",
    );
}

#[tokio::test]
async fn cross_session_inbox_post_list_read() {
    let state = fresh_state();
    let alice = register_session(&state, "codex").await;
    let bob = register_session(&state, "claude").await;

    let post = assert_ok(
        call(
            &state,
            "inbox.post",
            json!({
                "from_session": alice,
                "to_session": bob,
                "priority": "normal",
                "body": "Please review PR #42",
                "kind": "review-request",
            }),
        )
        .await,
        "inbox.post",
    );
    let mid = post["message_id"]
        .as_str()
        .expect("message_id")
        .to_string();

    // inbox.list returns Vec<InboxMessage> directly
    let list = assert_ok(
        call(&state, "inbox.list", json!({ "session_id": bob })).await,
        "inbox.list",
    );
    let msgs = list.as_array().expect("response is array");
    let mids: Vec<&str> = msgs.iter().filter_map(|m| m["message_id"].as_str()).collect();
    assert!(
        mids.contains(&mid.as_str()),
        "message_id {} not in list {:?}",
        mid,
        mids
    );

    let read = assert_ok(
        call(
            &state,
            "inbox.read",
            json!({ "session_id": bob, "message_id": mid }),
        )
        .await,
        "inbox.read",
    );
    assert_eq!(read["body"], "Please review PR #42");
}

#[tokio::test]
async fn state_set_then_get_round_trip() {
    let state = fresh_state();
    let sid = register_session(&state, "codex").await;

    // state.set uses flat fields (not nested {state: {...}})
    assert_ok(
        call(
            &state,
            "state.set",
            json!({
                "session_id": sid,
                "status": "working",
                "focus_file": "tests/daemon_e2e.rs",
                "focus_branch": "main",
            }),
        )
        .await,
        "state.set",
    );

    // state.get returns LiveState directly
    let got = assert_ok(
        call(&state, "state.get", json!({ "session_id": sid })).await,
        "state.get",
    );
    assert_eq!(got["session_id"], sid);
    assert_eq!(got["status"], "working");
    assert_eq!(got["focus_file"], "tests/daemon_e2e.rs");
    assert_eq!(got["focus_branch"], "main");
}

#[tokio::test]
async fn discover_agents_finds_active_sessions() {
    let state = fresh_state();
    let alice = register_session(&state, "codex").await;
    let _bob = register_session(&state, "claude").await;

    // Alice sets her focus
    assert_ok(
        call(
            &state,
            "state.set",
            json!({
                "session_id": alice,
                "status": "working",
                "focus_file": "tests/integration.rs",
                "focus_branch": "main",
            }),
        )
        .await,
        "state.set alice",
    );

    // discover.agents filters by path/branch/capabilities (not agent_type/status)
    let v = assert_ok(
        call(
            &state,
            "discover.agents",
            json!({ "path": "tests/integration.rs" }),
        )
        .await,
        "discover.agents",
    );
    let agents = v.as_array().expect("response is array");
    let ids: Vec<&str> = agents
        .iter()
        .filter_map(|a| a["session_id"].as_str())
        .collect();
    assert!(
        ids.contains(&alice.as_str()),
        "alice ({}) should match path=tests/integration.rs; got {:?}",
        alice,
        ids
    );
}
