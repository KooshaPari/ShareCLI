/// SidecarTickGate.swift — admission control for the supervisor's `tick()`.
///
/// Audit 1.8 (`PLAN.md:172-174`, `FINDINGS.md:40`): the bare
/// `guard !isTicking else { return }` + `defer { isTicking = false }` pair
/// turned any hung `tick()` into permanent supervisor death — the boolean had
/// no deadline, so a tick frozen inside a probe held the guard until process
/// exit and every later tick (and `ensureRunning()`) silently no-oped.
///
/// The gate supplies the two properties the plan asks for:
///
///   * **Bounded** — a tick in flight for `wedgedAfter`
///     (`SidecarSupervisorPolicy.tickSeconds × wedgedTickMultiple`) counts as
///     wedged; the next `begin()` takes over instead of refusing forever.
///   * **Self-resetting** — `exit(generation)` releases the gate when the
///     running tick finishes, and a superseded tick's late exit is a no-op
///     (generation counter), so the takeover cannot be clobbered by the tick
///     it replaced.
///
/// Time is injected, matching task 1.7's `PollClock` pattern, so tests
/// exercise the bound deterministically instead of sleeping on the wall clock.

import Foundation

public struct SidecarTickGate {
    /// How many `tickSeconds` a tick may run before it counts as wedged.
    /// 4 covers the slowest legitimate tick — health probe (3 s) + connect
    /// probe (1 s) + terminate grace (2 s) + a `ps` snapshot ≈ 7 s — at the
    /// default 3 s cadence (4 × 3 s = 12 s), while still letting a
    /// test-driven 0.2 s cadence take over in 0.8 s.
    public static let wedgedTickMultiple: Double = 4

    /// The takeover bound for this gate, in seconds.
    public let wedgedAfter: TimeInterval

    private let now: () -> Date
    private var inFlight: (generation: UInt64, startedAt: Date)?
    private var nextGeneration: UInt64 = 1

    public init(tickSeconds: TimeInterval, now: @escaping () -> Date = Date.init) {
        self.wedgedAfter = tickSeconds * Self.wedgedTickMultiple
        self.now = now
    }

    /// Ask to run one tick. Returns the generation to hand to `exit(_:)`, or
    /// `nil` while another tick is still inside its bound — ticks never stack.
    public mutating func begin() -> UInt64? {
        let started = now()
        if let current = inFlight, started.timeIntervalSince(current.startedAt) < wedgedAfter {
            return nil
        }
        // Idle, or the in-flight tick overran its bound: take over.
        let generation = nextGeneration
        nextGeneration += 1
        inFlight = (generation, started)
        return generation
    }

    /// Release the gate. Only the generation that owns the slot may release
    /// it; a superseded tick exiting late must not free the tick that took
    /// over from it.
    public mutating func exit(_ generation: UInt64) {
        guard inFlight?.generation == generation else { return }
        inFlight = nil
    }

    /// The generation currently in flight, if any (diagnostics/tests).
    public var inFlightGeneration: UInt64? { inFlight?.generation }
}
