// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Linux tray kill operator parity (AC-007.54)
//! FR: FR-007
//!
//! Linux tray MUST wire per-process Kill submenu + Kill All Managed to IPC
//! `process.kill` / `process.kill_all` (reference contract for Windows AC-007.54).

use sharecli_tray_linux::ipc::{kill, kill_all};
use sharecli_tray_windows::ipc::{IPC_METHOD_KILL, IPC_METHOD_KILL_ALL};

/// FR-007 / AC-007.54 — Linux tray kill IPC methods match handler contract.
#[test]
fn fr007_tray_linux_kill_ipc_methods() {
    let linux_ipc = include_str!("../crates/sharecli-tray-linux/src/ipc.rs");
    assert!(linux_ipc.contains("process.kill"), "Linux ipc MUST define process.kill (AC-007.54)");
    assert!(
        linux_ipc.contains("process.kill_all"),
        "Linux ipc MUST define process.kill_all (AC-007.54)"
    );
    assert_eq!(IPC_METHOD_KILL, "process.kill");
    assert_eq!(IPC_METHOD_KILL_ALL, "process.kill_all");
}

/// FR-007 / AC-007.54 — Linux tray menu wires Kill + Kill All Managed actions.
#[test]
fn fr007_tray_linux_kill_wires_tray_menu() {
    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("ipc::kill_all()"),
        "Linux tray MUST wire Kill All Managed (AC-007.54)"
    );
    assert!(
        main_rs.contains("ipc::kill(pid)"),
        "Linux tray MUST wire per-process Kill (AC-007.54)"
    );
    assert!(
        main_rs.contains("Kill All Managed"),
        "Linux tray MUST label Kill All Managed (AC-007.54)"
    );
    assert!(
        main_rs.contains(r#"label: "Kill".into()"#),
        "Linux tray MUST label per-process Kill (AC-007.54)"
    );

    // Compile-time parity: public kill helpers exist.
    let _kill: fn(u32) -> anyhow::Result<bool> = kill;
    let _kill_all: fn() -> anyhow::Result<bool> = kill_all;
}
