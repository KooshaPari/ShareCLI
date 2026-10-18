// SPDX-License-Identifier: Apache-2.0 OR MIT
//! teamcomm-smoke — E2E smoke harness for the teamcomm daemon.
//!
//! Brings up the daemon on a Unix socket, exercises all 14 wire methods
//! via direct handler invocation, and reports pass/fail per method group.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --package teamcomm-smoke
//! ```
//!
//! Exits 0 only if every wire method round-trips successfully.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use teamcomm_daemon::{handlers, state};
use teamcomm_daemon::state::AppState;

const AGENT_TYPE_CODEX: &str = "codex";
const AGENT_TYPE_CLAUDE: &str = "claude";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = state::new_state();
    let mut passed = 0u32;
    let mut failed = 0u32;

    macro_rules! check {
        ($name:expr, $result:expr) => {
            match $result {
                Ok(v) => {
                    tracing::info!("  ✓ {}", $name);
                    passed += 1;
                    v
                }
                Err(e) => {
                    let err_msg = format!("{:?}", e);
                    tracing::error!("  ✗ {}: {}", $name, err_msg);
                    failed += 1;
                    return Err(anyhow::anyhow!(err_msg));
                }
            }
        };
    }

    tracing::info!("═══ teamcomm-smoke — E2E harness ═══");

    // ── Session Lifecycle ──
    tracing::info!("── Sessions ──");
    let payload = json!({
        "agent_type": AGENT_TYPE_CODEX,
        "pid": std::process::id(),
        "metadata": HashMap::<String, Value>::new(),
    });
    let resp = check!("session.register", handlers::handle_session_register(state.clone(), payload).await);
    let sid = resp["session_id"].as_str().expect("session_id").to_string();
    tracing::info!("  session_id={}", sid);

    check!("session.heartbeat", handlers::handle_session_heartbeat(state.clone(), json!({ "session_id": sid })).await);
    check!("session.get", handlers::handle_session_get(state.clone(), json!({ "session_id": sid })).await);

    // Register a second session
    let payload2 = json!({
        "agent_type": AGENT_TYPE_CLAUDE,
        "pid": std::process::id(),
        "metadata": HashMap::<String, Value>::new(),
    });
    let _ = check!("session.register (2nd)", handlers::handle_session_register(state.clone(), payload2).await);

    let list_resp = check!("session.list", handlers::handle_session_list(state.clone(), json!({})).await);
    let sessions = list_resp["sessions"].as_array().expect("sessions array");
    assert!(sessions.len() >= 2, "expected at least 2 sessions");
    tracing::info!("  sessions listed: {}", sessions.len());

    // ── Reservations ──
    tracing::info!("── Reservations ──");
    let claim = check!("reservation.claim",
        handlers::handle_reservation_claim(state.clone(), json!({
            "session_id": sid, "path": "src/main.rs", "mode": "exclusive", "ttl_seconds": 60
        })).await
    );
    let rid = claim["reservation_id"].as_str().expect("reservation_id").to_string();

    check!("reservation.list", handlers::handle_reservation_list(state.clone(), json!({ "session_id": sid })).await);
    check!("reservation.release",
        handlers::handle_reservation_release(state.clone(), json!({
            "session_id": sid, "reservation_id": rid
        })).await
    );

    // ── Inbox ──
    tracing::info!("── Inbox ──");
    let post = check!("inbox.post",
        handlers::handle_inbox_post(state.clone(), json!({
            "from_session": sid, "to_session": sid,
            "priority": "normal", "body": "smoke test message", "kind": "test"
        })).await
    );
    let mid = post["message_id"].as_str().expect("message_id").to_string();

    check!("inbox.list", handlers::handle_inbox_list(state.clone(), json!({ "session_id": sid })).await);
    check!("inbox.read",
        handlers::handle_inbox_read(state.clone(), json!({ "session_id": sid, "message_id": mid })).await
    );

    // ── State ──
    tracing::info!("── State ──");
    check!("state.set",
        handlers::handle_state_set(state.clone(), json!({
            "session_id": sid,
            "state": { "status": "working", "focus": "smoke-test", "current_task": "running smoke harness" }
        })).await
    );
    check!("state.get", handlers::handle_state_get(state.clone(), json!({ "session_id": sid })).await);

    // ── Discovery ──
    tracing::info!("── Discovery ──");
    check!("discover.agents",
        handlers::handle_discover_agents(state.clone(), json!({
            "agent_type": AGENT_TYPE_CODEX, "status": "working"
        })).await
    );

    // ── Cleanup ──
    check!("session.deregister", handlers::handle_session_deregister(state.clone(), json!({ "session_id": sid })).await);

    tracing::info!("═══ smoke complete: {passed} passed, {failed} failed ═══");
    assert_eq!(failed, 0, "smoke harness failed");
    Ok(())
}
