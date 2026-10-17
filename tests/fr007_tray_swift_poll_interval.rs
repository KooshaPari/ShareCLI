//! FR-007 — Swift tray periodic poll parity (AC-007.53)
//! FR: FR-007
//!
//! Swift `AppState.startPolling` MUST use the same ~3 s cadence as Linux tray + Windows AC-007.52,
//! calling the AC-007.48 `monitoringReport` refresh path on each tick.

use sharecli_tray_linux::poll::TRAY_POLL_INTERVAL_SECS as LINUX_TRAY_POLL_INTERVAL_SECS;
use sharecli_tray_windows::poll::TRAY_POLL_INTERVAL_SECS as WIN_TRAY_POLL_INTERVAL_SECS;

/// FR-007 / AC-007.53 — Swift tray poll constant matches Linux/Windows 3 s cadence.
#[test]
fn fr007_tray_swift_poll_interval_seconds() {
    let tray_poll = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/TrayPoll.swift");
    assert!(
        tray_poll.contains("intervalSeconds: UInt64 = 3"),
        "TrayPoll.intervalSeconds MUST be 3 (AC-007.53)"
    );
    assert_eq!(LINUX_TRAY_POLL_INTERVAL_SECS, 3);
    assert_eq!(WIN_TRAY_POLL_INTERVAL_SECS, 3);
}

/// FR-007 / AC-007.53 — AppState polling loop references TrayPoll + monitoring.report refresh.
#[test]
fn fr007_tray_swift_poll_interval_wires_app_state() {
    let app_state = include_str!("../desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift");
    assert!(
        app_state.contains("TrayPoll.intervalSeconds"),
        "AppState MUST derive its poll cadence from TrayPoll.intervalSeconds (AC-007.53)"
    );
    assert!(app_state.contains("startPolling"), "AppState MUST expose startPolling (AC-007.53)");
    assert!(
        app_state.contains("monitoringReport()"),
        "AppState refresh MUST stay on monitoringReport (AC-007.48)"
    );
    assert!(
        app_state.contains("await self?.refresh()"),
        "AppState poll loop MUST call refresh each tick (AC-007.53)"
    );
}
