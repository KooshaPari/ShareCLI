// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — Linux tray periodic poll parity (AC-007.53)
//! FR: FR-007
//!
//! Linux tray MUST use the same ~3 s cadence as Windows AC-007.52 / Swift `AppState.startPolling`,
//! calling the AC-007.48 `monitoring.report` refresh path on each tick.

use sharecli_tray_linux::poll::{tray_poll_interval, TRAY_POLL_INTERVAL_SECS};
use sharecli_tray_windows::poll::TRAY_POLL_INTERVAL_SECS as WIN_TRAY_POLL_INTERVAL_SECS;

/// FR-007 / AC-007.53 — canonical Linux tray poll interval matches Windows 3 s cadence.
#[test]
fn fr007_tray_linux_poll_interval_seconds() {
    assert_eq!(TRAY_POLL_INTERVAL_SECS, 3, "Linux tray MUST poll every 3s (AC-007.53)");
    assert_eq!(tray_poll_interval(), std::time::Duration::from_secs(3));
    assert_eq!(
        TRAY_POLL_INTERVAL_SECS, WIN_TRAY_POLL_INTERVAL_SECS,
        "Linux/Windows tray poll MUST match (AC-007.53)"
    );
}

/// FR-007 / AC-007.53 — Linux tray loop references shared interval + refresh path.
#[test]
fn fr007_tray_linux_poll_interval_wires_tray_loop() {
    let poll_rs = include_str!("../crates/sharecli-tray-linux/src/poll.rs");
    assert!(
        poll_rs.contains("TRAY_POLL_INTERVAL_SECS: u64 = 3"),
        "Linux poll.rs MUST define TRAY_POLL_INTERVAL_SECS = 3 (AC-007.53)"
    );

    let main_rs = include_str!("../crates/sharecli-tray-linux/src/main.rs");
    assert!(
        main_rs.contains("sharecli_tray_linux::poll::tray_poll_interval"),
        "Linux tray loop MUST use tray_poll_interval() (AC-007.53)"
    );
    assert!(
        main_rs.contains("monitoring.report"),
        "Linux tray refresh MUST stay on monitoring.report (AC-007.48)"
    );
    assert!(
        main_rs.contains("handle.update(refresh)"),
        "Linux tray poll loop MUST invoke refresh (AC-007.53)"
    );
}
