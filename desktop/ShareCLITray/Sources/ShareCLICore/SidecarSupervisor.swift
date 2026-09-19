/// SidecarSupervisor.swift — keeps the `sharecli-ipc` sidecar alive for the whole
/// life of the tray app.
///
/// Why this exists
/// ---------------
/// `AppEntry.applicationDidFinishLaunching` used to probe the socket exactly once
/// and launch the sidecar only if that single probe failed. Nothing re-checked it
/// afterwards, so a sidecar that died later (killed, crashed, OOM) left the tray
/// polling a dead socket forever: the menu bar showed no data for hours and only
/// an app restart brought the sidecar back.
///
/// What this does
/// --------------
/// Runs its own supervision loop — deliberately independent of the UI poll loop,
/// so a wedged socket cannot stall recovery as well — and:
///
///   * probes liveness every `tickSeconds` with a raw `connect(2)`
///     (`SidecarSocketProbe`), which costs ~0.01 ms and puts no work on the
///     sidecar;
///   * relaunches the sidecar when the socket stops listening;
///   * throttles: at most one spawn attempt per `minAttemptInterval`, doubling to
///     `maxAttemptInterval` after consecutive failures;
///   * never spawns a second sidecar while a live `sharecli-ipc` process exists
///     (`SidecarProcessProbe`), and reaps its own child so no zombie accumulates;
///   * takes over a non-responding sidecar only when it is ours to take — our own
///     child, or an orphan reparented to launchd — and it stayed unresponsive
///     past `hangGraceSeconds`. A sidecar owned by another live tray is never
///     killed, it is reported instead;
///   * fails quietly when the binary is missing: the UI keeps working, shows
///     `unavailable`, and retries at the slow cadence (a later install recovers).
///
/// Bounded retries: after `failureBoundBeforeUnavailable` consecutive failed
/// attempts the state becomes `unavailable` and the interval pins to
/// `maxAttemptInterval` (120 s). Supervision never stops retrying, because the fix
/// has to be able to land without a restart; a successful probe returns the state
/// to `running`.

import Foundation
import Combine

@MainActor
public final class SidecarSupervisor: ObservableObject {
    public static let shared = SidecarSupervisor()

    public let binaryName = "sharecli-ipc"
    public let policy: SidecarSupervisorPolicy

    /// Current supervision state. Drives the popover's status line.
    @Published public private(set) var status: SidecarStatus = .idle

    /// PID of the sidecar this tray last launched (nil when it attached to one
    /// that was already running).
    @Published public private(set) var lastSpawnedPID: Int32?

    private let client: IPCClient
    private let socketPath: String

    private var loopTask: Task<Void, Never>?
    private var child: Process?
    private var cachedBinaryPath: String?
    private var lastKnownSidecarPID: Int32?

    // Attempt accounting / throttle state.
    private var lastAttemptAt: Date?
    private var awaitingStartupSince: Date?
    private var consecutiveFailures = 0
    private var firstUnreachableAt: Date?

    // Health watchdog state.
    private var lastHealthProbeAt: Date?
    private var unhealthyObservations = 0

    private var isTicking = false

    public init(
        client: IPCClient = IPCClient.defaultClient(),
        policy: SidecarSupervisorPolicy = SidecarSupervisorPolicy()
    ) {
        self.client = client
        self.socketPath = client.socketPath
        self.policy = policy
    }

    // MARK: - Lifecycle

    /// Start supervising. Idempotent. The loop owns its cadence, so it keeps
    /// working even while the UI poll loop is paused or blocked.
    public func start() {
        guard loopTask == nil else { return }
        loopTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                await self.tick()
                let interval = self.policy.tickSeconds
                try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
            }
        }
    }

    public func stop() {
        loopTask?.cancel()
        loopTask = nil
    }

    /// One-shot check for app launch: launch a sidecar if nothing is listening,
    /// and wait a bounded time for it to bind.
    ///
    /// Uses the cheap connect probe rather than a health probe so startup is never
    /// gated on a 0.4-2.1 s sysinfo scan. The loop's first health probe follows
    /// within one tick and corrects the state if the listener turns out to be
    /// wedged.
    @discardableResult
    public func ensureRunning() async -> SidecarStatus {
        if await SidecarSocketProbe.reachable(path: socketPath, timeout: policy.connectTimeout) {
            lastKnownSidecarPID = await Self.sidecarPIDs().first ?? lastKnownSidecarPID
            markHealthy()
            return status
        }

        await tick()

        if case .starting = status {
            let deadline = Date().addingTimeInterval(policy.startupWaitSeconds)
            while Date() < deadline {
                try? await Task.sleep(nanoseconds: 150_000_000)
                if await SidecarSocketProbe.reachable(path: socketPath, timeout: policy.connectTimeout) {
                    markHealthy()
                    break
                }
            }
        }
        return status
    }

    // MARK: - Loop

    private func tick() async {
        guard !isTicking else { return }
        isTicking = true
        defer { isTicking = false }

        let now = Date()

        guard await SidecarSocketProbe.reachable(path: socketPath, timeout: policy.connectTimeout) else {
            // Nothing is listening: the sidecar is gone.
            unhealthyObservations = 0
            await recover(now: now)
            return
        }

        guard healthProbeDue(now: now) else { return }

        lastHealthProbeAt = now
        if await SidecarSocketProbe.healthProbe(client: client, timeout: policy.healthProbeTimeout) {
            markHealthy()
            return
        }

        unhealthyObservations += 1
        guard unhealthyObservations >= policy.hungObservationThreshold else { return }
        // Accepts connections but does not answer: treat it like a dead sidecar.
        await recover(now: now)
    }

    private func healthProbeDue(now: Date) -> Bool {
        // Always confirm a fresh spawn promptly.
        if awaitingStartupSince != nil { return true }
        guard let last = lastHealthProbeAt else { return true }
        let interval = unhealthyObservations > 0
            ? policy.healthRetryInterval
            : policy.healthProbeInterval
        return now.timeIntervalSince(last) >= interval
    }

    /// Bring the sidecar back. Always throttled; never spawns a duplicate.
    private func recover(now: Date) async {
        if firstUnreachableAt == nil { firstUnreachableAt = now }

        // A sidecar we just launched gets a grace period to bind before its spawn
        // is written off.
        if let started = awaitingStartupSince {
            if now.timeIntervalSince(started) < policy.startupGraceSeconds {
                status = .starting
                return
            }
            awaitingStartupSince = nil
            consecutiveFailures += 1
        }

        // Throttle: one attempt per interval, backing off on repeated failure.
        let interval = currentAttemptInterval()
        if let last = lastAttemptAt, now.timeIntervalSince(last) < interval {
            if consecutiveFailures >= policy.failureBoundBeforeUnavailable {
                status = .unavailable(reason: slowRetryReason())
            } else {
                status = .restarting(
                    attempt: consecutiveFailures + 1,
                    nextAttemptIn: max(last.addingTimeInterval(interval).timeIntervalSince(now), 0)
                )
            }
            return
        }

        // Never start a second sidecar while one is still running.
        let live = await Self.liveSidecars(binaryName: binaryName)
        if let running = live.first {
            lastKnownSidecarPID = running.pid
            let unreachableFor = now.timeIntervalSince(firstUnreachableAt ?? now)
            guard unreachableFor >= policy.hangGraceSeconds else {
                status = .restarting(
                    attempt: consecutiveFailures + 1,
                    nextAttemptIn: policy.hangGraceSeconds - unreachableFor
                )
                return
            }
            let candidates = SidecarProcessProbe.takeoverCandidates(
                in: live,
                ownPID: Self.ownPID,
                binaryName: binaryName
            )
            guard !candidates.isEmpty else {
                lastAttemptAt = now
                status = .unavailable(
                    reason: "sidecar pid \(running.pid) is not responding and is not owned by this tray"
                )
                return
            }
            await SidecarProcessProbe.terminate(pids: candidates.map(\.pid), graceSeconds: 2)
            let remaining = await Self.liveSidecars(binaryName: binaryName)
            if let blocker = remaining.first {
                lastAttemptAt = now
                status = .unavailable(reason: "sidecar pid \(blocker.pid) did not exit")
                return
            }
        }

        guard let exe = await resolveBinaryPath() else {
            lastAttemptAt = now
            consecutiveFailures = max(consecutiveFailures, policy.failureBoundBeforeUnavailable)
            status = .unavailable(reason: "\(binaryName) binary not found (bundle or PATH)")
            return
        }

        lastAttemptAt = now
        if spawn(exe) {
            awaitingStartupSince = now
            status = .starting
        } else {
            consecutiveFailures += 1
            status = .unavailable(reason: "failed to launch \(exe)")
        }
    }

    private func currentAttemptInterval() -> TimeInterval {
        if consecutiveFailures >= policy.failureBoundBeforeUnavailable {
            return policy.maxAttemptInterval
        }
        return policy.interval(afterConsecutiveFailures: consecutiveFailures)
    }

    private func slowRetryReason() -> String {
        "no answer after \(consecutiveFailures) restart attempts; retrying every \(Int(policy.maxAttemptInterval))s"
    }

    private func markHealthy() {
        consecutiveFailures = 0
        firstUnreachableAt = nil
        awaitingStartupSince = nil
        lastAttemptAt = nil
        unhealthyObservations = 0
        status = .running(pid: child?.processIdentifier ?? lastKnownSidecarPID)
    }

    // MARK: - Spawn

    /// Launch the sidecar. Returns false when `Process.run` threw.
    private func spawn(_ exe: String) -> Bool {
        let proc = Process()
        let url = URL(fileURLWithPath: exe)
        proc.executableURL = url
        // Run from the binary's own directory, matching the installer layout and
        // the Windows/Linux trays' sidecar launch.
        proc.currentDirectoryURL = url.deletingLastPathComponent()

        do {
            try proc.run()
        } catch {
            return false
        }

        child = proc
        lastKnownSidecarPID = proc.processIdentifier
        lastSpawnedPID = proc.processIdentifier

        // Reap on a background thread: `waitUntilExit` must not run on the main
        // thread, and without it every dead sidecar lingers as a zombie.
        DispatchQueue.global(qos: .utility).async { [weak self] in
            proc.waitUntilExit()
            Task { @MainActor in
                guard let self, self.child === proc else { return }
                self.child = nil
            }
        }
        return true
    }

    private func resolveBinaryPath() async -> String? {
        if let cached = cachedBinaryPath, FileManager.default.isExecutableFile(atPath: cached) {
            return cached
        }
        let name = binaryName
        let resolved: String? = await withCheckedContinuation { cont in
            DispatchQueue.global(qos: .utility).async {
                cont.resume(returning: Self.lookupBinaryPath(name))
            }
        }
        if let resolved { cachedBinaryPath = resolved }
        return resolved
    }

    /// Bundle first (shipped layout), then PATH (developer builds). A missing
    /// binary is not cached as a negative result, so installing it later is picked
    /// up on the next slow retry without an app restart.
    public nonisolated static func lookupBinaryPath(_ name: String) -> String? {
        let bundleExe = "\(Bundle.main.bundlePath)/Contents/Resources/bin/\(name)"
        if FileManager.default.isExecutableFile(atPath: bundleExe) { return bundleExe }
        let path = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/local/bin:/usr/bin:/bin"
        for dir in path.split(separator: ":") {
            let candidate = "\(dir)/\(name)"
            if FileManager.default.isExecutableFile(atPath: candidate) { return candidate }
        }
        return nil
    }

    private nonisolated static func liveSidecars(binaryName: String) async -> [SidecarProcessEntry] {
        SidecarProcessProbe.liveSidecars(
            in: await SidecarProcessProbe.snapshotAsync(),
            binaryName: binaryName
        )
    }

    private nonisolated static func sidecarPIDs() async -> [Int32] {
        SidecarProcessProbe.liveSidecars(in: await SidecarProcessProbe.snapshotAsync())
            .map(\.pid)
    }

    public nonisolated static var ownPID: Int32 {
        ProcessInfo.processInfo.processIdentifier
    }
}
