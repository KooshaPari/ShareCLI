import Foundation

/// The scheduling decision behind the tray's background poll loop.
///
/// Extracted from `AppState.startPolling()` for task 1.7 so cadence can be
/// tested without an `IPCClient`. Everything here is pure: no clock is read,
/// so these tests are exact rather than wall-clock tolerant.
public enum PollCadence {
    /// When the next poll should *start*.
    ///
    /// - Parameters:
    ///   - previous: the instant the poll that just finished was scheduled to
    ///     start (its grid point, not the instant work actually began).
    ///   - interval: nominal period.
    ///   - now: the instant work finished.
    /// - Returns: `previous + interval` when that is still in the future,
    ///   otherwise `now`.
    ///
    /// The `now` branch is what keeps the cadence honest when a poll overruns
    /// its period: it coalesces the missed beats instead of waking immediately
    /// to catch up, and it never adds `interval` on top of the overrun — which
    /// is exactly the drift this replaced.
    public static func deadline(
        previous: ContinuousClock.Instant,
        interval: Duration,
        now: ContinuousClock.Instant
    ) -> ContinuousClock.Instant {
        let candidate = previous.advanced(by: interval)
        return candidate > now ? candidate : now
    }
}

/// The time source `PollLoop` schedules on.
///
/// A concrete wrapper rather than `any Clock` because Swift's clock protocol is
/// generically constrained over both `Instant` and `Duration`, and an
/// existential loses the inference the loop needs. The wrapper keeps the loop
/// on a plain, `Sendable` value while still letting tests supply a clock that
/// only advances when slept upon — which is what makes scheduling assertions
/// exact instead of jitter-tolerant.
public struct PollClock: Sendable {
    private let nowProvider: @Sendable () -> ContinuousClock.Instant
    private let sleeper: @Sendable (ContinuousClock.Instant) async throws -> Void

    public init(
        now: @escaping @Sendable () -> ContinuousClock.Instant,
        sleepUntil: @escaping @Sendable (ContinuousClock.Instant) async throws -> Void
    ) {
        self.nowProvider = now
        self.sleeper = sleepUntil
    }

    /// A real monotonic clock. Monotonic on purpose: a wall-clock adjustment
    /// must not shift the poll grid.
    public static let continuous = PollClock(
        now: { ContinuousClock().now },
        sleepUntil: { try await ContinuousClock().sleep(until: $0) }
    )

    public var now: ContinuousClock.Instant { nowProvider() }

    public func sleep(until deadline: ContinuousClock.Instant) async throws {
        try await sleeper(deadline)
    }
}

/// A cooperative, cancellation-honouring poll loop.
///
/// The work closure is injected so tests can drive cadence directly; `AppState`
/// supplies `{ await self.refresh() }`. The clock is injectable for the same
/// reason and defaults to `PollClock.continuous`.
public struct PollLoop: Sendable {
    /// Nominal period between poll *starts*.
    public let interval: Duration

    private let work: @Sendable () async -> Void
    private let clock: PollClock

    public init(
        interval: Duration,
        clock: PollClock = .continuous,
        work: @escaping @Sendable () async -> Void
    ) {
        self.interval = interval
        self.clock = clock
        self.work = work
    }

    /// Runs `work` until the surrounding task is cancelled.
    ///
    /// The first poll starts immediately. Every later poll is scheduled from
    /// the previous poll's **scheduled** start rather than from the instant
    /// its work happened to finish, so the effective period is `interval` and
    /// not `interval + work`. Scheduling sleeps on the injected clock, which is
    /// also what makes cancellation terminate the loop promptly instead of
    /// waiting out a full interval.
    public func run() async {
        var deadline = clock.now
        while !Task.isCancelled {
            await work()
            deadline = PollCadence.deadline(previous: deadline, interval: interval, now: clock.now)
            do {
                try await clock.sleep(until: deadline)
            } catch {
                return // CancellationError — the task is going away.
            }
        }
    }
}
