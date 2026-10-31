// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Tray periodic poll interval (FR-007 / AC-007.53).
//!
//! WinUI `TrayWindow` and Swift `AppState` poll every 3 s; the Linux tray loop MUST use the
//! same interval via `tray_poll_interval()` wired to the AC-007.48 `monitoring.report` refresh path.

use std::time::Duration;

/// Operator tray refresh cadence shared across Linux, macOS, and Windows (AC-007.53).
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
