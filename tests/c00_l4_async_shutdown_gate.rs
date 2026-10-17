// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 L4 / FR-003 — structured async shutdown + scoped spawn env (CancellationToken).
//!
//! Evidence: `src/shutdown.rs`, `src/commands/serve.rs`, `docs/ops/async-shutdown.md`.

#[test]
fn c00_l4_shutdown_module_present() {
    let src = include_str!("../src/shutdown.rs");
    assert!(src.contains("CancellationToken"), "shutdown.rs must use CancellationToken");
    assert!(src.contains("serve_shutdown_signal"), "shutdown.rs must export serve_shutdown_signal");
}

#[test]
fn c00_l4_serve_uses_graceful_shutdown() {
    let serve_rs = include_str!("../src/commands/serve.rs");
    assert!(
        serve_rs.contains("with_graceful_shutdown"),
        "serve.rs must call axum::serve with_graceful_shutdown"
    );
    assert!(serve_rs.contains("serve_shutdown_signal"), "serve.rs must wire serve_shutdown_signal");
    assert!(
        serve_rs.contains("thermal_cancel"),
        "serve.rs must fan out CancellationToken to thermal poller"
    );
}

#[test]
fn c00_l4_spawn_env_blocking_scope() {
    let runtime_rs = include_str!("../src/runtime.rs");
    assert!(
        runtime_rs.contains("spawn_blocking"),
        "runtime.rs must scope env overrides in spawn_blocking"
    );
    assert!(runtime_rs.contains("EnvGuard"), "runtime.rs must retain EnvGuard RAII restore");
}

#[test]
fn c00_l4_async_shutdown_docs_present() {
    let doc = include_str!("../docs/ops/async-shutdown.md");
    assert!(doc.contains("CancellationToken"), "async-shutdown.md must document CancellationToken");
    assert!(
        doc.contains("with_graceful_shutdown"),
        "async-shutdown.md must document graceful HTTP shutdown"
    );
}

#[test]
fn c00_l4_tokio_util_dependency() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("tokio-util"),
        "Cargo.toml must declare tokio-util for CancellationToken"
    );
}
