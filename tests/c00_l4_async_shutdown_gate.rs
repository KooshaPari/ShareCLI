//! C00 L4 / FR-003 — structured async shutdown + scoped spawn env (CancellationToken).
//!
//! Evidence: `src/shutdown.rs`, `src/commands/serve.rs`, `docs/ops/async-shutdown.md`.
//!
//! FR: FR-003

#[test]
fn c00_l4_shutdown_module_present() {
    let src = include_str!("../src/shutdown.rs");
    assert!(src.contains("CancellationToken"), "shutdown.rs must use CancellationToken");
    assert!(src.contains("serve_shutdown_signal"), "shutdown.rs must export serve_shutdown_signal");
}

#[test]
fn c00_l4_serve_uses_graceful_shutdown() {
    // Post-decomposition: the original monolithic `src/commands/serve.rs` was
    // split into a `commands/serve/` module directory. The implementation
    // lives in `commands/serve/run.rs` (axum::serve entry, graceful-shutdown
    // wiring, thermal cancel fan-out). Concatenate every `commands/serve/`
    // .rs file so this static check survives future sub-splits.
    let serve_rs = std::fs::read_dir("src/commands/serve")
        .expect("src/commands/serve must exist post-decomposition")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(|p| std::fs::read_to_string(&p).expect("failed to read serve submodule"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        serve_rs.contains("with_graceful_shutdown"),
        "serve module must call axum::serve with_graceful_shutdown"
    );
    assert!(
        serve_rs.contains("serve_shutdown_signal"),
        "serve module must wire serve_shutdown_signal"
    );
    assert!(
        serve_rs.contains("thermal_cancel"),
        "serve module must fan out CancellationToken to thermal poller"
    );
}

#[test]
fn c00_l4_spawn_env_blocking_scope() {
    // Post-decomposition: `src/runtime.rs` was split into a `runtime/` module
    // directory. `spawn_blocking` lives in `runtime/pool.rs` and
    // `runtime/process_info.rs`; `EnvGuard` is defined in `process_info.rs`
    // and re-exported from `mod.rs`. Concatenate every `runtime/` .rs file
    // so the check survives future sub-splits.
    let runtime_rs = std::fs::read_dir("src/runtime")
        .expect("src/runtime must exist post-decomposition")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(|p| std::fs::read_to_string(&p).expect("failed to read runtime submodule"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        runtime_rs.contains("spawn_blocking"),
        "runtime module must scope env overrides in spawn_blocking"
    );
    assert!(runtime_rs.contains("EnvGuard"), "runtime module must retain EnvGuard RAII restore");
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
