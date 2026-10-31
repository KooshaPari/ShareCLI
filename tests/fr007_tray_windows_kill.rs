// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Windows tray kill operator parity (AC-007.54)
//! FR: FR-007
//!
//! WinUI tray MUST wire per-process Kill + Kill All Managed to IPC `process.kill` /
//! `process.kill_all`, then refresh via AC-007.51 `RefreshDataAsync` (parity with Linux
//! tray + Swift `AppState.kill` / `killAll`).

use sharecli_tray_linux::ipc::{kill, kill_all};
use sharecli_tray_windows::ipc::{
    kill_all_request_json, kill_request_json, IPC_METHOD_KILL, IPC_METHOD_KILL_ALL,
};

/// FR-007 / AC-007.54 — Windows tray kill IPC methods match Linux/Swift contract.
#[test]
fn fr007_tray_windows_kill_ipc_methods() {
    assert_eq!(IPC_METHOD_KILL, "process.kill");
    assert_eq!(IPC_METHOD_KILL_ALL, "process.kill_all");
}

/// FR-007 / AC-007.54 — Windows kill request wire matches Linux tray `ipc::kill`.
#[test]
fn fr007_tray_windows_kill_request_wire_parity() {
    let win = kill_request_json(3, 99);
    let linux = serde_json::json!({ "id": 3, "method": "process.kill", "params": { "pid": 99 } })
        .to_string();
    assert_eq!(win, linux, "Windows kill request MUST match Linux wire (AC-007.54)");
}

/// FR-007 / AC-007.54 — Windows kill_all request wire matches Linux tray `ipc::kill_all`.
#[test]
fn fr007_tray_windows_kill_all_request_wire_parity() {
    let win = kill_all_request_json(4);
    let linux =
        serde_json::json!({ "id": 4, "method": "process.kill_all", "params": {} }).to_string();
    assert_eq!(win, linux, "Windows kill_all request MUST match Linux wire (AC-007.54)");
}

/// FR-007 / AC-007.54 — WinUI tray wires Kill + Kill All + post-kill refresh.
#[test]
fn fr007_tray_windows_kill_wires_tray_window() {
    let ipc_kill = include_str!("../windows/ShareCLITray/IpcKill.cs");
    assert!(ipc_kill.contains("process.kill"), "IpcKill MUST call process.kill (AC-007.54)");
    assert!(
        ipc_kill.contains("process.kill_all"),
        "IpcKill MUST call process.kill_all (AC-007.54)"
    );
    assert!(ipc_kill.contains("pid"), "IpcKill MUST pass pid param (AC-007.54)");

    let tray_cs = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        tray_cs.contains("IpcKill.TryKill"),
        "TrayWindow MUST invoke IpcKill.TryKill (AC-007.54)"
    );
    assert!(
        tray_cs.contains("IpcKill.TryKillAll"),
        "TrayWindow MUST invoke IpcKill.TryKillAll (AC-007.54)"
    );
    assert!(
        tray_cs.contains("await RefreshDataAsync()"),
        "TrayWindow MUST refresh after kill (AC-007.54 / AC-007.51)"
    );

    let tray_xaml = include_str!("../windows/ShareCLITray/TrayWindow.xaml");
    assert!(
        tray_xaml.contains("OnKillProcessClick"),
        "TrayWindow MUST expose per-process Kill action (AC-007.54)"
    );
    assert!(
        tray_xaml.contains("Kill All Managed"),
        "TrayWindow MUST expose Kill All Managed action (AC-007.54)"
    );
}

/// FR-007 / AC-007.54 — Linux tray kill helpers remain canonical reference contract.
#[test]
fn fr007_tray_windows_kill_linux_reference_parity() {
    // Linux ipc.rs encodes the same methods used by sharecli-ipc handler.
    let linux_ipc = include_str!("../crates/sharecli-tray-linux/src/ipc.rs");
    assert!(linux_ipc.contains(r#"call("process.kill""#));
    assert!(linux_ipc.contains(r#"call("process.kill_all""#));

    // Request builders must stay aligned with Linux param shapes.
    let _ = kill_request_json(1, 1);
    let _ = kill_all_request_json(2);
    // Linux public API exists for runtime tray (compile-time parity).
    let _kill: fn(u32) -> anyhow::Result<bool> = kill;
    let _kill_all: fn() -> anyhow::Result<bool> = kill_all;
}
