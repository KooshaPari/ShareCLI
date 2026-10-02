import XCTest

@testable import ShareCLICore

/// Task 1.7 — effective cadence in the Swift poll loop.
///
/// `FINDINGS.md` [HIGH]: the loop was `await work(); sleep(interval)`, so the
/// effective period was `interval + work` rather than `interval`. With a 3 s
/// configured period and a slow snapshot, every sparkline sample landed late
/// and the charts (`LineMark(x: sample.timestamp)`) were spaced by the drift,
/// not by the period the UI claims.
///
/// The loop-level tests below record the instant each poll *starts* and assert
/// on the mean spacing. Each threshold is the **midpoint between the two
/// regimes** the defect can produce (`interval` vs `interval + work`), so the
/// verdict cannot be decided by scheduler jitter: the broken loop sits on the
/// wrong side of the midpoint by construction and jitter only pushes it
/// further, while the fixed loop keeps half the separation as headroom.
/// Thresholds are chosen wide (200-300 ms) because these tests must also hold
/// on a loaded development host.
final class PollLoopTests: XCTestCase {

    // MARK: - Pure scheduling decision

    func testDeadlineIsScheduledIntervalAheadWhenWorkFitsInsideThePeriod() {
        let clock = ContinuousClock()
        let start = clock.now
        let interval = Duration.seconds(3)

        let deadline = PollCadence.deadline(
            previous: start,
            interval: interval,
            now: start + .milliseconds(200)
        )

        XCTAssertEqual(deadline, start + interval)
    }

    func testDeadlineCoalescesAnOverrunInsteadOfStackingAnotherInterval() {
        let clock = ContinuousClock()
        let start = clock.now
        let interval = Duration.seconds(3)

        // Work took 4 s against a 3 s period: the next poll must be "now",
        // not "now + 3 s", which is how the old loop reached `interval + work`.
        let now = start + .seconds(4)
        let deadline = PollCadence.deadline(previous: start, interval: interval, now: now)

        XCTAssertEqual(deadline, now)
        XCTAssertLessThan(deadline, now + interval)
    }

    func testDeadlineAnchorsToTheScheduledGridNotToWorkCompletion() {
        let clock = ContinuousClock()
        let grid = clock.now
        let interval = Duration.seconds(3)

        // First poll ran at `grid`, finished 200 ms later -> next starts at
        // grid + 3 s, i.e. exactly 3 s after the *start*, not after the end.
        let deadline = PollCadence.deadline(previous: grid, interval: interval, now: grid + .milliseconds(200))
        XCTAssertEqual(deadline, grid + interval)

        // And the returned deadline becomes the next anchor, so the grid
        // advances by whole intervals regardless of per-poll duration.
        let second = PollCadence.deadline(previous: deadline, interval: interval, now: deadline + .milliseconds(900))
        XCTAssertEqual(second, grid + .seconds(6))
    }

    // MARK: - Loop-level cadence

    func testEffectivePeriodIsTheIntervalNotIntervalPlusWork() async {
        let interval = Duration.milliseconds(500)
        let work = Duration.milliseconds(400)
        let recorder = StartRecorder()

        let loop = PollLoop(interval: interval) {
            recorder.record(ContinuousClock().now)
            try? await Task.sleep(for: work)
        }

        let task = Task { await loop.run() }
        let samples = await recorder.wait(for: 7, timeout: .seconds(20))
        task.cancel()
        await task.value

        XCTAssertGreaterThanOrEqual(samples.count, 7, "expected at least 7 polls to judge cadence")
        let mean = Self.meanSpacingSeconds(samples)
        // The two regimes are 0.500 s (fixed) and 0.900 s (broken); the
        // threshold is their midpoint, so 0.200 s of scheduler overshoot per
        // poll is tolerated before a green run could fail — and the broken
        // loop sits 0.200 s on the wrong side by construction.
        let threshold = 0.500 + 0.400 / 2
        XCTAssertLessThan(
            mean,
            threshold,
            "mean poll period was \(mean)s — expected ≈\(interval.secondsDescription)s, "
                + "not interval+work = \(interval.secondsDescription)+\(work.secondsDescription)"
        )
    }

    func testAnOverrunPollDoesNotDelayTheFollowingPollByAnotherInterval() async {
        let interval = Duration.milliseconds(600)
        let work = Duration.milliseconds(1000)
        let recorder = StartRecorder()

        let loop = PollLoop(interval: interval) {
            recorder.record(ContinuousClock().now)
            try? await Task.sleep(for: work)
        }

        let task = Task { await loop.run() }
        let samples = await recorder.wait(for: 4, timeout: .seconds(20))
        task.cancel()
        await task.value

        XCTAssertGreaterThanOrEqual(samples.count, 4, "expected at least 4 polls to judge cadence")
        let mean = Self.meanSpacingSeconds(samples)
        // Regimes: 1.000 s (fixed — back-to-back) vs 1.600 s (broken — work
        // plus another full interval). Midpoint leaves 0.300 s of headroom.
        let threshold = 1.000 + 0.600 / 2
        XCTAssertLessThan(
            mean,
            threshold,
            "overrunning poll pushed the next start out by a full interval (mean \(mean)s)"
        )
    }

    func testFirstPollStartsImmediately() async {
        let recorder = StartRecorder()
        let loop = PollLoop(interval: .seconds(30)) {
            recorder.record(ContinuousClock().now)
        }

        let task = Task { await loop.run() }
        let samples = await recorder.wait(for: 1, timeout: .seconds(5))
        task.cancel()
        await task.value

        XCTAssertEqual(samples.count, 1)
    }

    func testCancellationEndsTheLoopInsteadOfWaitingOutTheInterval() async {
        let recorder = StartRecorder()
        let loop = PollLoop(interval: .seconds(30)) {
            recorder.record(ContinuousClock().now)
        }

        let task = Task { await loop.run() }
        _ = await recorder.wait(for: 1, timeout: .seconds(5))
        let before = ContinuousClock().now
        task.cancel()
        await task.value
        let elapsed = ContinuousClock().now - before

        XCTAssertLessThan(elapsed.secondsValue, 5.0, "loop survived cancellation for \(elapsed.secondsDescription)")
    }

    // MARK: - Deterministic scheduling (manual clock)

    // These two lock the schedule to the millisecond. They exist only because
    // `PollLoop` now takes its clock: the pre-fix loop slept on `Task.sleep`,
    // which no test can intercept, so the wall-clock tests above are the ones
    // that carry the red evidence.

    func testEachPollStartsOnTheIntervalGridNotOnWorkCompletion() async {
        let state = ManualClockState(now: ContinuousClock().now, maxSleeps: 2)
        let recorder = StartRecorder()
        let interval = Duration.seconds(3)

        let loop = PollLoop(interval: interval, clock: state.pollClock) {
            recorder.record(state.now)
            state.advance(by: .milliseconds(500)) // work takes half a period
        }

        await loop.run()
        let samples = recorder.snapshot

        XCTAssertEqual(samples.count, 3)
        XCTAssertEqual(samples[1], samples[0] + interval, "second poll must start 3 s after the first, not 3.5 s")
        XCTAssertEqual(samples[2], samples[0] + interval + interval, "third poll must stay on the 3 s grid")
    }

    func testOverrunPollCoalescesInsteadOfStackingAnotherInterval() async {
        let state = ManualClockState(now: ContinuousClock().now, maxSleeps: 1)
        let recorder = StartRecorder()
        let interval = Duration.seconds(3)

        let loop = PollLoop(interval: interval, clock: state.pollClock) {
            recorder.record(state.now)
            state.advance(by: .seconds(4)) // work overruns the 3 s period
        }

        await loop.run()
        let samples = recorder.snapshot

        XCTAssertEqual(samples.count, 2)
        XCTAssertEqual(
            samples[1],
            samples[0] + .seconds(4),
            "next poll must start as soon as the overrun ends (3 s old loop would say 7 s)"
        )
    }

    // MARK: - Helpers

    private static func meanSpacingSeconds(_ stamps: [ContinuousClock.Instant]) -> Double {
        guard stamps.count >= 2 else { return .infinity }
        let span = (stamps[stamps.count - 1] - stamps[0]).secondsValue
        return span / Double(stamps.count - 1)
    }
}

/// Thread-safe record of poll start instants. The loop runs off the main
/// actor, so the recorder cannot be a plain array property.
private final class StartRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var stamps: [ContinuousClock.Instant] = []

    func record(_ instant: ContinuousClock.Instant) {
        lock.lock()
        stamps.append(instant)
        lock.unlock()
    }

    var snapshot: [ContinuousClock.Instant] {
        lock.lock()
        defer { lock.unlock() }
        return stamps
    }

    func wait(for count: Int, timeout: Duration) async -> [ContinuousClock.Instant] {
        let clock = ContinuousClock()
        let deadline = clock.now + timeout
        while clock.now < deadline {
            let current = snapshot
            if current.count >= count { return current }
            try? await Task.sleep(for: .milliseconds(25))
        }
        return snapshot
    }
}

private extension Duration {
    var secondsValue: Double {
        Double(components.seconds) + Double(components.attoseconds) / 1e18
    }

    var secondsDescription: String {
        String(format: "%.3f", secondsValue)
    }
}

/// A clock that only moves when it is slept upon, so `PollLoop` scheduling can
/// be asserted exactly instead of within a jitter tolerance.
///
/// `maxSleeps` bounds the loop: once that many advances have happened the next
/// sleep throws, which is how a deterministic test terminates a loop that would
/// otherwise run forever on a clock that never really advances.
private final class ManualClockState: @unchecked Sendable {
    private let lock = NSLock()
    private var current: ContinuousClock.Instant
    private var successfulSleeps = 0
    private let maxSleeps: Int

    init(now: ContinuousClock.Instant, maxSleeps: Int) {
        self.current = now
        self.maxSleeps = maxSleeps
    }

    var now: ContinuousClock.Instant {
        lock.lock()
        defer { lock.unlock() }
        return current
    }

    func advance(by duration: Duration) {
        lock.lock()
        current = current.advanced(by: duration)
        lock.unlock()
    }

    /// - Returns: `false` once the test's budget of sleeps is spent.
    func trySleep(until deadline: ContinuousClock.Instant) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard successfulSleeps < maxSleeps else { return false }
        successfulSleeps += 1
        if deadline > current { current = deadline }
        return true
    }

    /// Sleep that never waits in real time: it jumps the manual clock to the
    /// deadline, and throws once `maxSleeps` is spent so the loop under test
    /// terminates instead of spinning on a clock that never really advances.
    func sleep(until deadline: ContinuousClock.Instant) async throws {
        try Task.checkCancellation()
        guard trySleep(until: deadline) else {
            throw CancellationError() // budget spent — end the loop under test
        }
    }

    /// A `PollClock` backed by this state.
    var pollClock: PollClock {
        PollClock(now: { self.now }, sleepUntil: { try await self.sleep(until: $0) })
    }
}
