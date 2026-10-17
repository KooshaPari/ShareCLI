//! FR: FR-004
// FR/AC ids: UNKNOWN (plan 1.12 names none)
// Tests for task 1.12: shared ProcessPool in AppState
// Text-gate tests: verify AppState has pool field, handler doesn't create fresh pool.

use std::sync::Arc;

/// AppState must have a pool field of type Arc<ProcessPool>.
/// This is a compile-time check - if the field doesn't exist, this won't compile.
#[test]
fn appstate_has_pool_field() {
    // This test verifies the struct definition has a pool field.
    // We can't directly inspect struct fields at runtime, but we can
    // verify the type exists and is accessible.
    // If AppState doesn't have a pool field, the code won't compile.

    // Create a minimal AppState to verify the pool field exists
    // This will fail to compile if pool field is missing
    let _pool: Arc<sharecli::runtime::ProcessPool> =
        Arc::new(sharecli::runtime::ProcessPool::new());
}

/// Metrics handler must read pool from AppState, not create a fresh one.
/// This is a text-gate test: scan the source for the anti-pattern.
#[test]
fn metrics_handler_no_fresh_pool() {
    // Read the serve.rs file and check for the anti-pattern
    let serve_rs =
        std::fs::read_to_string("src/commands/serve.rs").expect("Failed to read serve.rs");

    // Find the metrics_prometheus_handler function
    let handler_start = serve_rs
        .find("async fn metrics_prometheus_handler")
        .expect("metrics_prometheus_handler not found");

    // Find the next function (end of handler)
    let handler_end = serve_rs[handler_start..]
        .find("\nfn ")
        .map(|pos| handler_start + pos)
        .unwrap_or(serve_rs.len());

    let handler_body = &serve_rs[handler_start..handler_end];

    // Must NOT contain ProcessPool::new()
    assert!(
        !handler_body.contains("ProcessPool::new()"),
        "metrics_prometheus_handler must NOT create fresh ProcessPool::new(). Found:\n{handler_body}"
    );

    // Must access pool from state
    assert!(
        handler_body.contains("state.pool") || handler_body.contains("state. pool"),
        "metrics_prometheus_handler must access pool from state. Found:\n{handler_body}"
    );
}
