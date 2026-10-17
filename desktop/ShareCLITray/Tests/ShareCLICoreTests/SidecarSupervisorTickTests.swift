import Darwin
import Foundation
import XCTest

@testable import ShareCLICore

/// Audit 1.8 / PLAN.md:172-174 — "Swift supervisor tick is bounded and
/// self-resetting … bound tick with deadline; reset on exit."
///
/// FINDINGS.md:40 states the defect: "`guard !isTicking else { return }` +
/// `defer { isTicking = false }` turns any hung `tick()` into permanent
/// supervisor death; concurrent `ensureRunning()` then returns stale state."
///
/// The behavioural test reproduces exactly that against the pristine code: a
/// real loop tick is frozen inside the health probe (a listener that accepts
/// and never answers), the sidecar path then becomes unreachable, and a later
/// `ensureRunning()` must still drive supervision. With the bare boolean guard
/// the frozen tick holds `isTicking` until its 30 s probe deadline, every later
/// `tick()` no-ops, and `ensureRunning()` hands back the untouched `.idle` —
/// the "permanent supervisor death" red.
final class SidecarSupervisorTickTests: XCTestCase {

    @MainActor
    func testFrozenTickDoesNotPermanentlyBlockSupervision() async throws {
        // A listener that accepts connections and never answers: the tick's
        // `health.status` probe hangs until its policy deadline (30 s), which
        // is the "hung tick" from the finding.
        let server = LineSidecarServer()
        server.setResponder { _ in nil }
        defer { server.stop() }

        var policy = SidecarSupervisorPolicy()
        policy.tickSeconds = 0.2 // fast loop cadence so the test observes many ticks
        policy.healthProbeTimeout = 30 // freeze window far beyond the assertion wait

        let client = IPCClient(socketPath: server.path, timeout: 60)
        let supervisor = SidecarSupervisor(client: client, policy: policy)

        // A recovery tick must never find a real `sharecli-ipc` to spawn.
        let savedPath = ProcessInfo.processInfo.environment["PATH"] ?? ""
        setenv("PATH", "/sharecli-tick-tests-no-such-dir", 1)
        defer { setenv("PATH", savedPath, 1) }

        supervisor.start()
        defer { supervisor.stop() }

        // The loop's first tick must pass the re-entry guard and reach the
        // socket probe — an accepted connection is the observable proof that
        // the guard was passed (the probe runs inside `tick()`), so from here
        // `isTicking` is held until the probe's deadline.
        let accepted = await withCheckedContinuation { (cont: CheckedContinuation<Bool, Never>) in
            DispatchQueue.global(qos: .userInitiated).async {
                cont.resume(returning: server.waitUntilAccepted(1, seconds: 15))
            }
        }
        XCTAssertTrue(accepted, "reproduction premise: the first tick never reached the socket")

        // While that tick is frozen in flight, the sidecar becomes unreachable.
        unlink(server.path)

        // 10 × policy.tickSeconds (2.0 s): far longer than any bounded tick's
        // takeover deadline derived from the same policy, far shorter than the
        // 30 s freeze window, so the frozen tick is provably still in flight
        // when the assertion runs. The green run measures the actual takeover
        // latency against this wait.
        try await Task.sleep(nanoseconds: 2_000_000_000)

        let status = await supervisor.ensureRunning()
        XCTAssertNotEqual(
            status,
            .idle,
            "ensureRunning() handed back the untouched .idle state: the frozen tick "
                + "still owns the re-entry guard, so supervision is dead until restart (audit 1.8)"
        )
    }

    // MARK: - Mechanism (pins how the bound and the reset work)

    /// Deterministic time source, exactly as task 1.7's `PollClock` pattern:
    /// the gate's bound is exercised by advancing a manual clock, never by
    /// sleeping on the wall clock.
    private final class ManualClock: @unchecked Sendable {
        private let lock = NSLock()
        private var current: Date

        init(_ start: Date = Date(timeIntervalSinceReferenceDate: 0)) {
            current = start
        }

        var now: Date {
            lock.lock()
            defer { lock.unlock() }
            return current
        }

        func advance(by seconds: TimeInterval) {
            lock.lock()
            current = current.addingTimeInterval(seconds)
            lock.unlock()
        }
    }

    func testTickIsRefusedWhileAnotherIsInFlightWithinItsBound() {
        let clock = ManualClock()
        var gate = SidecarTickGate(tickSeconds: 3, now: { clock.now })

        let first = gate.begin()
        XCTAssertNotNil(first, "an idle gate admits the first tick")

        clock.advance(by: 11) // < 4 × 3 s bound: still inside the deadline
        XCTAssertNil(gate.begin(), "a tick inside its bound must be refused, not stacked")
    }

    func testGateAdmitsAgainAfterTheRunningTickExits() {
        let clock = ManualClock()
        var gate = SidecarTickGate(tickSeconds: 3, now: { clock.now })

        let first = gate.begin()
        XCTAssertNotNil(first)
        gate.exit(first!)
        XCTAssertNotNil(gate.begin(), "exit must reset the gate for the next tick")
    }

    func testOverrunningTickIsTakenOverAfterTheBoundAndStaleExitCannotReleaseIt() {
        let clock = ManualClock()
        var gate = SidecarTickGate(tickSeconds: 3, now: { clock.now })

        let wedged = gate.begin()
        XCTAssertNotNil(wedged)

        clock.advance(by: 12) // == 4 × 3 s: the in-flight tick is now wedged
        let takeover = gate.begin()
        XCTAssertNotNil(
            takeover,
            "a tick in flight past the bound is wedged; supervision must continue"
        )
        XCTAssertNotEqual(takeover, wedged, "the takeover must be a fresh generation")

        // The superseded tick finally returns from the dead; its late exit
        // must not free the generation that replaced it.
        gate.exit(wedged!)
        XCTAssertNil(gate.begin(), "the stale tick's exit must not release the live generation")

        gate.exit(takeover!)
        XCTAssertNotNil(gate.begin(), "the live generation's exit resets the gate")
    }

    func testBoundDerivesFromThePolicyTickSeconds() {
        let policy = SidecarSupervisorPolicy()
        let gate = SidecarTickGate(tickSeconds: policy.tickSeconds)

        XCTAssertEqual(gate.wedgedAfter, policy.tickSeconds * SidecarTickGate.wedgedTickMultiple)
        XCTAssertGreaterThanOrEqual(
            SidecarTickGate.wedgedTickMultiple,
            3,
            "the bound must exceed the slowest legitimate tick (~7 s of probes "
                + "and terminate grace at a 3 s cadence) before takeover is safe"
        )
    }
}
