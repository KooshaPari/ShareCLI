// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Windows tray periodic poll parity (AC-007.52)
//! FR: FR-007
//!
//! Linux tray and Swift `AppState` poll ~every 3 s; WinUI MUST wire an equivalent timer loop
//! calling the AC-007.51 `monitoring.report` refresh path.

use sharecli_tray_windows::poll::{tray_poll_interval, TRAY_POLL_INTERVAL_SECS};

/// FR-007 / AC-007.52 — canonical tray poll interval matches Linux/Swift 3 s cadence.
#[test]
fn fr007_tray_windows_poll_interval_seconds() {
    assert_eq!(TRAY_POLL_INTERVAL_SECS, 3, "Windows tray MUST poll every 3s (AC-007.52)");
    assert_eq!(tray_poll_interval(), std::time::Duration::from_secs(3));
}

/// FR-007 / AC-007.52 — WinUI timer wiring references shared interval + refresh path.
#[test]
fn fr007_tray_windows_poll_interval_wires_winui_timer() {
    let cs_poll = include_str!("../windows/ShareCLITray/TrayPoll.cs");
    assert!(
        cs_poll.contains("IntervalSeconds = 3"),
        "TrayPoll.IntervalSeconds MUST be 3 (AC-007.52)"
    );

    let cs_window = include_str!("../windows/ShareCLITray/TrayWindow.xaml.cs");
    assert!(
        cs_window.contains("DispatcherQueueTimer"),
        "TrayWindow MUST use DispatcherQueueTimer (AC-007.52)"
    );
    assert!(
        cs_window.contains("TrayPoll.IntervalSeconds"),
        "TrayWindow timer MUST reference TrayPoll.IntervalSeconds (AC-007.52)"
    );
    assert!(
        cs_window.contains("RefreshDataAsync"),
        "TrayWindow timer MUST call RefreshDataAsync (AC-007.52 / AC-007.51 path)"
    );
    assert!(
        cs_window.contains("monitoring.report"),
        "TrayWindow refresh MUST stay on monitoring.report (AC-007.51)"
    );
}
