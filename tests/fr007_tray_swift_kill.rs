// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Swift tray kill operator parity (AC-007.54)
//! FR: FR-007
//!
//! Swift `AppState.kill` / `killAll` MUST call IPC `process.kill` / `process.kill_all`
//! and refresh (reference contract for Windows AC-007.54).

use sharecli_tray_windows::ipc::{IPC_METHOD_KILL, IPC_METHOD_KILL_ALL};

/// FR-007 / AC-007.54 — Swift IPCClient kill methods match handler contract.
#[test]
fn fr007_tray_swift_kill_ipc_methods() {
    let ipc_client = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift");
    assert!(
        ipc_client.contains(r#""process.kill""#),
        "IPCClient MUST call process.kill (AC-007.54)"
    );
    assert!(
        ipc_client.contains(r#""process.kill_all""#),
        "IPCClient MUST call process.kill_all (AC-007.54)"
    );
    assert_eq!(IPC_METHOD_KILL, "process.kill");
    assert_eq!(IPC_METHOD_KILL_ALL, "process.kill_all");
}

/// FR-007 / AC-007.54 — Swift AppState wires kill + post-kill refresh.
#[test]
fn fr007_tray_swift_kill_wires_app_state() {
    let app_state = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift");
    assert!(app_state.contains("client.kill(pid:"), "AppState MUST call client.kill (AC-007.54)");
    assert!(
        app_state.contains("client.killAll()"),
        "AppState MUST call client.killAll (AC-007.54)"
    );
    assert!(
        app_state.contains("await refresh()"),
        "AppState MUST refresh after kill (AC-007.54 / AC-007.48)"
    );

    let popover =
        include_str!("../desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift");
    assert!(
        popover.contains("state.kill(pid:"),
        "Tray popover MUST expose per-process kill (AC-007.54)"
    );
    assert!(popover.contains("state.killAll()"), "Tray popover MUST expose Kill All (AC-007.54)");
}
