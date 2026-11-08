// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Tray periodic poll interval (FR-007 / AC-007.52).
//!
//! Linux tray and Swift `AppState` poll every 3 s; WinUI `TrayWindow` MUST use the same
//! interval via `DispatcherQueueTimer` wired to `TrayPoll.IntervalSeconds`.

use std::time::Duration;

/// Operator tray refresh cadence shared across Linux, macOS, and Windows (AC-007.52).
pub const TRAY_POLL_INTERVAL_SECS: u64 = 3;

/// Canonical poll interval for tray `monitoring.report` refresh loops.
pub fn tray_poll_interval() -> Duration {
    Duration::from_secs(TRAY_POLL_INTERVAL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_poll_interval_is_three_seconds() {
        assert_eq!(TRAY_POLL_INTERVAL_SECS, 3);
        assert_eq!(tray_poll_interval(), Duration::from_secs(3));
    }
}
