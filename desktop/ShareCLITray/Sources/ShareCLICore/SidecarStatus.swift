/// SidecarStatus.swift — the supervision contract: observable state plus the
/// tunables the supervision loop obeys.
///
/// Split out of `SidecarSupervisor.swift` so the state vocabulary and the
/// backoff maths (unit-tested in `SidecarSupervisorTests`) do not pull in the
/// process-spawning code.

import Foundation

/// Operator-visible state of the `sharecli-ipc` sidecar.
///
/// `running` means the socket answered a full health probe. The launch path also
/// reports `running` after a bare successful `connect`; the loop re-verifies
/// within one tick and corrects the state if the listener is wedged.
public enum SidecarStatus: Equatable, Sendable {
    case idle
    case running(pid: Int32?)
    case starting
    case restarting(attempt: Int, nextAttemptIn: TimeInterval)
    case unavailable(reason: String)

    public var isRunning: Bool {
        if case .running = self { return true }
        return false
    }

    /// Short label for the tray popover / dashboard header.
    public var shortLabel: String {
        switch self {
        case .idle: return "starting"
        case .running: return "running"
        case .starting: return "starting"
        case .restarting(let attempt, _): return "restarting (attempt \(attempt))"
        case .unavailable: return "unavailable"
        }
    }

    /// Non-nil only when the sidecar needs operator attention. `running` and
    /// `idle` stay quiet so the popover does not flicker on every restart.
    public var operatorMessage: String? {
        switch self {
        case .idle, .running:
            return nil
        case .starting:
            return "sidecar starting…"
        case .restarting(let attempt, let next):
            return "sidecar down — relaunch attempt \(attempt) in \(Int(next.rounded(.up)))s"
        case .unavailable(let reason):
            return "sidecar unavailable — \(reason)"
        }
    }
}

/// Tunables for the supervision loop. Kept as a value type so the backoff maths
/// can be exercised without spawning anything.
public struct SidecarSupervisorPolicy: Equatable, Sendable {
    /// Loop cadence. Matches `TrayPoll.intervalSeconds`.
    public var tickSeconds: TimeInterval = 3
    /// Floor between two spawn attempts: never hot-loop respawns.
    public var minAttemptInterval: TimeInterval = 10
    /// Ceiling for the backoff, including the `unavailable` state's cadence.
    public var maxAttemptInterval: TimeInterval = 120
    public var backoffFactor: Double = 2
    /// Consecutive failed attempts before the state becomes `unavailable`.
    /// Retries continue at `maxAttemptInterval`; supervision is never abandoned,
    /// because the fix (binary installed, pressure relieved) must be able to land
    /// without an app restart.
    public var failureBoundBeforeUnavailable: Int = 5
    /// Grace for a freshly spawned sidecar to bind and answer before the spawn
    /// counts as failed.
    public var startupGraceSeconds: TimeInterval = 5
    /// How long the launch path waits for a just-spawned sidecar to bind.
    public var startupWaitSeconds: TimeInterval = 3
    /// A live sidecar that never answers must stay unresponsive this long before
    /// it is taken over.
    public var hangGraceSeconds: TimeInterval = 15
    /// Full health probe cadence while `running`. The probe is the watchdog
    /// against a listener that accepts connections but never answers.
    public var healthProbeInterval: TimeInterval = 30
    /// Shortened probe cadence once a probe failed, so a hang is caught quickly.
    public var healthRetryInterval: TimeInterval = 5
    /// Bound on one health probe. Under load `health.status` measured 0.4-2.1 s
    /// on this host, so 3 s avoids calling a busy-but-healthy sidecar hung.
    public var healthProbeTimeout: TimeInterval = 3
    /// Consecutive failed probes before a reachable listener counts as hung.
    public var hungObservationThreshold: Int = 2
    /// Bound on the `connect(2)` liveness probe.
    public var connectTimeout: TimeInterval = 1

    public init() {}

    /// Interval before the next spawn attempt, given `n` consecutive failures
    /// since the last healthy sidecar. 0 -> the floor; grows by `backoffFactor`;
    /// caps at `maxAttemptInterval`.
    public func interval(afterConsecutiveFailures n: Int) -> TimeInterval {
        guard n > 0 else { return minAttemptInterval }
        let grown = minAttemptInterval * pow(backoffFactor, Double(n - 1))
        return min(max(grown, minAttemptInterval), maxAttemptInterval)
    }
}
