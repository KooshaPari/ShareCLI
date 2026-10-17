import Foundation
import XCTest
@testable import ShareCLICore

/// REGRESSION GUARD for the task 1.7 wiring of `AppState.startPolling()`.
///
/// Task 1.7 refactored `startPolling()` to delegate its loop to the new
/// `PollLoop` type (Sources/ShareCLICore/PollLoop.swift). This test is the
/// end-to-end guard for that wiring: it constructs a fresh `AppState`, calls
/// `startPolling()`, and requires live sidecar round-trips to surface as
/// published state -- `isConnected == true`, at least two `hostWatchHistory`
/// samples (one appended per successful poll), and no recorded `lastError`.
/// A regression where polling never starts, `refresh()` is never called, or
/// the connection state never updates fails here.
///
/// No pre-fix red is possible for this guard: the original inline loop had no
/// test seam, so there was never a state in which this test could have been
/// written against the old code and observed failing. The red evidence for
/// 1.7 lives in `PollLoopTests.swift` (wall-clock cadence tests).
///
/// Gated exactly like `IPCClientRealPathTests`: it requires a live, isolated
/// `sharecli-ipc` sidecar and skips when unset. Note that `AppState.client`
/// is `IPCClient.defaultClient()`, which reads `SHARECLI_IPC_SOCK` (NOT
/// `SHARECLI_TEST_IPC_SOCK`), so this gate uses that same variable:
///
///     SHARECLI_IPC_SOCK=/tmp/iso.sock swift test --filter AppStatePollTests
final class AppStatePollTests: XCTestCase {

    /// `startPolling()` must actually start the loop, the loop must call
    /// `refresh()`, and `refresh()` must update the published connection
    /// state -- observed through the real sidecar, not a synthetic server.
    @MainActor
    func test_startPolling_drives_live_state_from_the_sidecar() async throws {
        guard let path = ProcessInfo.processInfo.environment["SHARECLI_IPC_SOCK"] else {
            throw XCTSkip("SHARECLI_IPC_SOCK not set; start an isolated sharecli-ipc")
        }

        // A fresh instance, not AppState.shared: this test owns the polling
        // lifecycle and must not disturb other tests.
        let state = AppState()
        state.startPolling()
        // ALWAYS stop polling, even when the assertions below fail or the
        // test is cancelled mid-wait.
        defer { state.stopPolling() }

        // Poll cadence is TrayPoll.intervalSeconds (3 s) and the first poll
        // runs immediately, so the second hostWatchHistory sample lands at
        // ~3 s. Budget 10 s (~3 intervals) with a ~100 ms poll keeps the
        // wait responsive while tolerating a loaded host.
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline {
            if state.isConnected && state.hostWatchHistory.count >= 2 {
                break
            }
            try await Task.sleep(nanoseconds: 100_000_000)
        }

        XCTAssertTrue(
            state.isConnected,
            "startPolling never marked the app connected against sidecar at \(path)"
        )
        XCTAssertGreaterThanOrEqual(
            state.hostWatchHistory.count, 2,
            "startPolling must append one hostWatchHistory sample per successful "
                + "poll (interval \(TrayPoll.intervalSeconds)s); got "
                + "\(state.hostWatchHistory.count) against sidecar at \(path)"
        )
        XCTAssertNil(
            state.lastError,
            "live polling against sidecar at \(path) must not record an error"
        )
    }
}
