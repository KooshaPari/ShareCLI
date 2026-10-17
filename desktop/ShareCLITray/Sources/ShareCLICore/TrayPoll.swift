/// Periodic tray poll cadence (FR-007 / AC-007.53).
///
/// MUST match `sharecli_tray_linux::poll::TRAY_POLL_INTERVAL_SECS` and
/// `sharecli_tray_windows::poll::TRAY_POLL_INTERVAL_SECS` (Linux/Windows parity).
public enum TrayPoll {
    public static let intervalSeconds: UInt64 = 3
}
