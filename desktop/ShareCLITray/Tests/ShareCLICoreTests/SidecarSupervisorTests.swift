import XCTest

@testable import ShareCLICore

/// Headless tests for the sidecar supervision rules.
///
/// These deliberately cover the decision logic and the `ps` parsing rather than
/// the live relaunch: spawning a real `sharecli-ipc` from a test would fight the
/// tray that owns the socket. The end-to-end relaunch is verified by killing the
/// running sidecar and watching the tray PID show up as its new parent.
final class SidecarSupervisorTests: XCTestCase {

    // MARK: - Backoff

    func testIntervalStartsAtFloorAndDoesNotBackOffBeforeFirstFailure() {
        let policy = SidecarSupervisorPolicy()
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 0), policy.minAttemptInterval)
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 1), policy.minAttemptInterval)
    }

    func testIntervalDoublesThenCapsAtMaximum() {
        let policy = SidecarSupervisorPolicy()
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 2), 20)
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 3), 40)
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 4), 80)
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 5), policy.maxAttemptInterval)
        XCTAssertEqual(policy.interval(afterConsecutiveFailures: 50), policy.maxAttemptInterval)
    }

    /// The throttle has to keep at least the floor between two spawn attempts,
    /// otherwise a fast poll cadence could hot-loop respawns.
    func testThrottleFloorIsNotBelowTheMinimumAttemptInterval() {
        let policy = SidecarSupervisorPolicy()
        for failures in 0...20 {
            XCTAssertGreaterThanOrEqual(
                policy.interval(afterConsecutiveFailures: failures),
                policy.minAttemptInterval
            )
        }
        XCTAssertGreaterThanOrEqual(policy.tickSeconds, 1)
    }

    // MARK: - ps parsing

    private let samplePS = """
          1     0 S    /sbin/launchd
      57809     1 S    /Users/x/Applications/ShareCLITray.app/Contents/MacOS/ShareCLITray
      57817 57809 S    /Users/x/Applications/ShareCLITray.app/Contents/Resources/bin/sharecli-ipc
      57818 57817 Z    sharecli-ipc
      59000     1 S    /opt/homebrew/bin/sharecli-ipc
      59001     1 S    /Applications/Some App.app/Contents/MacOS/Some App
        notapid here
    """

    func testParseReadsPidPpidStateAndFullPath() {
        let entries = SidecarProcessProbe.parse(samplePS)
        XCTAssertEqual(entries.count, 6, "the malformed row must be dropped")

        let tray = try? XCTUnwrap(entries.first { $0.pid == 57809 })
        XCTAssertEqual(tray?.ppid, 1)
        XCTAssertEqual(tray?.name, "ShareCLITray")

        let sidecar = entries.first { $0.pid == 57817 }
        XCTAssertEqual(sidecar?.ppid, 57809)
        XCTAssertEqual(sidecar?.name, "sharecli-ipc", "comm is a full path on macOS")
        XCTAssertEqual(sidecar?.state, "S")
    }

    func testParseKeepsPathsContainingSpaces() {
        let entries = SidecarProcessProbe.parse(samplePS)
        XCTAssertEqual(
            entries.first { $0.pid == 59001 }?.comm,
            "/Applications/Some App.app/Contents/MacOS/Some App"
        )
    }

    /// A zombie is a dead previous sidecar that has not been reaped yet. Counting
    /// it as live would make the tray refuse to relaunch, which is the bug this
    /// whole file exists to avoid.
    func testZombieSidecarIsNotTreatedAsLive() {
        let live = SidecarProcessProbe.liveSidecars(in: SidecarProcessProbe.parse(samplePS))
        XCTAssertFalse(live.contains { $0.pid == 57818 }, "a zombie must not count as running")
        XCTAssertEqual(live.map(\.pid).sorted(), [57817, 59000])
    }

    func testZombieFlagFollowsTheStateColumn() {
        XCTAssertTrue(SidecarProcessEntry(pid: 1, ppid: 2, state: "Z", comm: "x").isZombie)
        XCTAssertTrue(SidecarProcessEntry(pid: 1, ppid: 2, state: "Z+", comm: "x").isZombie)
        XCTAssertFalse(SidecarProcessEntry(pid: 1, ppid: 2, state: "S", comm: "x").isZombie)
    }

    // MARK: - Ownership

    func testTakeoverCandidatesCoverOwnChildrenAndOrphansOnly() {
        let entries = SidecarProcessProbe.parse(samplePS)
        let candidates = SidecarProcessProbe.takeoverCandidates(in: entries, ownPID: 57809)
        XCTAssertEqual(
            candidates.map(\.pid).sorted(),
            [57817, 59000],
            "our child and the launchd-reparented orphan are ours; a child of another live process is not"
        )
    }

    func testTakeoverRefusesASidecarOwnedByAnotherLiveProcess() {
        let entries = [
            SidecarProcessEntry(pid: 42, ppid: 777, state: "S", comm: "/usr/local/bin/sharecli-ipc")
        ]
        XCTAssertTrue(
            SidecarProcessProbe.takeoverCandidates(in: entries, ownPID: 57809).isEmpty,
            "another live tray owns that sidecar; killing it would break that tray"
        )
    }

    // MARK: - Status vocabulary

    func testRunningAndIdleStayQuietInThePopover() {
        XCTAssertNil(SidecarStatus.running(pid: 1).operatorMessage)
        XCTAssertNil(SidecarStatus.idle.operatorMessage)
        XCTAssertTrue(SidecarStatus.running(pid: 1).isRunning)
    }

    func testRestartingAndUnavailableAreSurfacedToTheOperator() {
        let restarting = SidecarStatus.restarting(attempt: 2, nextAttemptIn: 12.4)
        XCTAssertTrue(restarting.operatorMessage?.contains("attempt 2") == true)
        XCTAssertTrue(restarting.operatorMessage?.contains("13s") == true, "rounds up")
        XCTAssertFalse(restarting.isRunning)

        let unavailable = SidecarStatus.unavailable(reason: "sharecli-ipc binary not found (bundle or PATH)")
        XCTAssertTrue(unavailable.operatorMessage?.contains("not found") == true)
        XCTAssertEqual(unavailable.shortLabel, "unavailable")
    }

    // MARK: - Binary lookup

    func testLookupOfAMissingBinaryFailsQuietly() {
        XCTAssertNil(
            SidecarSupervisor.lookupBinaryPath("sharecli-ipc-does-not-exist-\(UUID().uuidString)"),
            "an absent sidecar must produce nil, not a crash or a bogus path"
        )
    }
}
