/// One sample of fleet-wide aggregates. Captured per poll and pushed
/// onto `AppState.fleetHistory`. Used by the Processes page Trends
/// subpage to draw memory + CPU + process-count sparklines.
public struct FleetSample: Identifiable, Hashable {
    public var id: Date { timestamp }
    public let timestamp: Date
    public let totalProcesses: Int
    public let totalMemoryMB: UInt64
    public let usedMemoryMB: UInt64
    public let cpuAvgPercent: Float
    public let poolHealthy: Bool

    public init(
        timestamp: Date,
        totalProcesses: Int,
        totalMemoryMB: UInt64,
        usedMemoryMB: UInt64,
        cpuAvgPercent: Float,
        poolHealthy: Bool
    ) {
        self.timestamp = timestamp
        self.totalProcesses = totalProcesses
        self.totalMemoryMB = totalMemoryMB
        self.usedMemoryMB = usedMemoryMB
        self.cpuAvgPercent = cpuAvgPercent
        self.poolHealthy = poolHealthy
    }
}

/// AppState.swift — Observable state for the tray popover + main window.
///
/// Polls the IPC server on `TrayPoll.intervalSeconds` cadence for live data via a single
/// `monitoring.report` snapshot (AC-007.48 / AC-007.72): gate/host_watch + process inventory
/// + embedded pool/status in one round-trip.
///
/// Also maintains two rolling in-memory windows used by the dashboard:
///   - `hostWatchHistory` — last `hostWatchHistoryCap` samples of `HostResourceWatchJson`
///     (PR 6 of the dashboard expansion: Host watch sparklines)
///   - `gateDecisionHistory` — last `gateDecisionHistoryCap` gate decisions, each tagged
///     with the wall-clock time it was observed (PR 6 of the dashboard expansion: thermal
///     gate panel "last 20 gate decisions" log)

import Foundation
import Combine

/// One sample of host resource watch telemetry. Captured on every successful
/// `monitoring.report` poll and pushed onto `AppState.hostWatchHistory`.
///
/// Field semantics match `HostResourceWatchJson` (the wire shape from the
/// sidecar's `monitoring.report` envelope).
public struct HostWatchSample: Identifiable, Hashable {
    public var id: Date { timestamp }
    public let timestamp: Date
    public let fd_count: UInt64
    public let net_rx_bytes: UInt64
    public let net_tx_bytes: UInt64
    public let mem_rss_bytes: UInt64
    public let load_1m: Double

    public init(
        timestamp: Date,
        fd_count: UInt64,
        net_rx_bytes: UInt64,
        net_tx_bytes: UInt64,
        mem_rss_bytes: UInt64,
        load_1m: Double
    ) {
        self.timestamp = timestamp
        self.fd_count = fd_count
        self.net_rx_bytes = net_rx_bytes
        self.net_tx_bytes = net_tx_bytes
        self.mem_rss_bytes = mem_rss_bytes
        self.load_1m = load_1m
    }

    public init(timestamp: Date, host: HostResourceWatchJson) {
        self.init(
            timestamp: timestamp,
            fd_count: host.fd_count,
            net_rx_bytes: host.net_rx_bytes,
            net_tx_bytes: host.net_tx_bytes,
            mem_rss_bytes: host.mem_rss_bytes,
            load_1m: host.load_1m
        )
    }
}

/// One observed gate decision. Captured on every successful poll when the
/// gate decision string is non-empty (it can be "UNAVAILABLE" when the
/// sidecar is offline — those are recorded too).
public struct GateDecisionSample: Identifiable, Hashable {
    public var id: Date { timestamp }
    public let timestamp: Date
    public let thermalPressure: String
    public let detectedAgents: Int
    public let agentTotalRssBytes: UInt64
    public let agentContention: String
    public let gateDecision: String

    public init(timestamp: Date, gate: GateStatusSnapshot) {
        self.timestamp = timestamp
        self.thermalPressure = gate.thermal_pressure
        self.detectedAgents = gate.detected_agents
        self.agentTotalRssBytes = gate.agent_total_rss_bytes
        self.agentContention = gate.agent_contention
        self.gateDecision = gate.gate_decision
    }
}

/// One row of the Spawn history (P1-7 of processes-page expansion).
/// Persisted as JSON to `~/Library/Application Support/sharecli/spawn-history.json`
/// so the in-app Spawn history survives app restarts.
///
/// Captures what the user submitted (command + args + project + harness +
/// cwd, all of which are sent over `process.spawn`) plus the outcome
/// (success/failure + spawned PID + error).
public struct SpawnHistoryEntry: Codable, Identifiable, Hashable {
    public let id: UUID
    public let timestamp: Date
    public let command: String
    public let args: [String]
    public let project: String?
    public let harness: String?
    public let workingDir: String
    public let succeeded: Bool
    public let spawnedPID: UInt32?
    public let errorMessage: String?

    public init(
        id: UUID = UUID(),
        timestamp: Date = Date(),
        command: String,
        args: [String],
        project: String?,
        harness: String?,
        workingDir: String,
        succeeded: Bool,
        spawnedPID: UInt32?,
        errorMessage: String?
    ) {
        self.id = id
        self.timestamp = timestamp
        self.command = command
        self.args = args
        self.project = project
        self.harness = harness
        self.workingDir = workingDir
        self.succeeded = succeeded
        self.spawnedPID = spawnedPID
        self.errorMessage = errorMessage
    }
}

@MainActor
public final class AppState: ObservableObject {
    /// Cap for the host watch rolling window. The spec calls for a 60s × 1s
    /// window (60 entries); TrayPoll polls every 3s so the actual window is
    /// ~3 minutes of samples. The cap keeps memory bounded either way.
    public static let hostWatchHistoryCap = 60

    /// Cap for the gate decision rolling window (last 20 decisions, per the
    /// Health page Thermal gate panel).
    public static let gateDecisionHistoryCap = 20

    @Published public var processes: [ProcessSummary] = []
    @Published public var health: HealthSnapshot?
    @Published public var poolStatus: PoolSnapshot?
    @Published public var statusSnapshot: StatusSnapshot?
    @Published public var poolEffectiveness: PoolEffectivenessSnapshot?
    @Published public var lastError: String?
    @Published public var isConnected: Bool = false

    /// Rolling window of host resource watch samples (most recent last).
    @Published public var hostWatchHistory: [HostWatchSample] = []

    /// Rolling window of gate decision observations (most recent last).
    @Published public var gateDecisionHistory: [GateDecisionSample] = []

    /// Rolling window of pool effectiveness samples (most recent last).
    /// Cap mirrors host_watch history; effectiveness samples are cheap to keep.
    public static let poolEffectivenessHistoryCap = 60
    @Published public var poolEffectivenessHistory: [PoolEffectivenessSnapshot] = []

    /// Per-agent RSS history ring buffer (PR 5 of dashboard expansion plan).
    /// Each value is an RSS sample in bytes, taken from successive
    /// `monitoring.report` polls. Capped at `agentRSSHistoryCap` per PID.
    /// Older entries are evicted FIFO.
    public static let agentRSSHistoryCap = 60
    @Published public var agentRSSHistory: [UInt32: [UInt64]] = [:]

    /// Fleet-wide aggregate sample (PR 2b of processes-page expansion).
    /// Captured on every successful poll. Used by the Trends subpage to
    /// render memory + CPU + process-count sparklines over the last 60
    /// polls (~3 minutes at the default 3s interval).
    public static let fleetHistoryCap = 60
    @Published public var fleetHistory: [FleetSample] = []

    /// Spawn history (P1-7). Ring buffer of the last `spawnHistoryCap`
    /// spawn attempts. Persisted as JSON to
    /// `~/Library/Application Support/sharecli/spawn-history.json`
    /// so it survives app restarts. SpawnView reads `spawnHistory` to
    /// render the recent-attempts list and re-submit button.
    public static let spawnHistoryCap = 50
    /// Persistent JSON log of recent spawn attempts (cap 50).
    /// Populated by `recordSpawn(_:)`, loaded on init.
    @Published public var spawnHistory: [SpawnHistoryEntry] = []

    /// Last observed set of fleet processes keyed by PID — used by the
    /// fleet-diff capture in `refresh()` to compute added/removed rows
    /// for the Diff subpage (P1-8 of dashboard expansion plan).
    /// Not published (subscribers only care about the diff result).
    private var previousProcesses: [UInt32: ProcessSummary] = [:]

    /// Most recent fleet diff (added / removed / changed). Posted via
    /// `sharecliFleetChanged` after every successful refresh.
    @Published public var fleetChange: FleetChange = FleetChange()

    /// Rolling history of fleet changes (cap 60). Used by DiffView for
    /// "what changed in the last 60s" timeline.
    @Published public var fleetChangeHistory: [FleetChange] = []
    public static let fleetChangeHistoryCap = 60

    /// File URL for the persisted spawn-history JSON.
    private static let spawnHistoryURL: URL = {
        let fm = FileManager.default
        let dir = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first!
            .appendingPathComponent("sharecli", isDirectory: true)
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("spawn-history.json")
    }()

    /// IPC client reference (process-page expansion: Spawn subpage needs
    /// to call process.spawn directly). Kept `public` so views can
    /// invoke one-shot IPC methods without round-tripping through AppState.
    public let client: IPCClient = IPCClient.defaultClient()

    /// Cmdline cache (PR 5 of dashboard expansion plan). Keyed by PID so
    /// navigating between agents on the Agents page doesn't re-fetch
    /// the same cmdline. TTL-free for now — the entry stays until the
    /// PID disappears from the agents list (then we evict).
    @Published public var cmdlineCache: [UInt32: ProcessCmdline] = [:]

    /// When true, the polling loop is suspended. The user can pause
    /// background refreshes (useful when they're investigating a
    /// specific process and don't want data drifting under them). The
    /// dashboard's Pause/Resume button + the right-click NSMenu both
    /// toggle this. Resume re-runs `startPolling()` so a fresh poll
    /// cycle begins immediately.
    @Published public var isPaused: Bool = false

    private var pollTask: Task<Void, Never>?

    /// Shared singleton used by TrayMenuController's MenuAction (which
    /// cannot hold a reference to the AppState passed via initializer).
    /// The per-window `@StateObject` instances (DashboardView / TrayPopoverView)
    /// are separate objects; this shared instance is only for menu-action dispatch
    /// and will reflect the same underlying data because they share the same
    /// IPC client and polling task.
    public static let shared = AppState()

    public init() {
        self.loadSpawnHistory()
    }

    public func startPolling() {
        guard !isPaused else { return }
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(nanoseconds: TrayPoll.intervalNanoseconds)
            }
        }
    }

    public func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }

    /// Pause background polling. Idempotent. Safe to call when not
    /// started. Distinct from `stopPolling()` which tears down the
    /// task entirely; `pausePolling()` leaves the task alive so
    /// `resumePolling()` is a no-op-free transition.
    public func pausePolling() {
        isPaused = true
        pollTask?.cancel()
        pollTask = nil
    }

    /// Resume polling. If already running, no-op. If `isPaused`, flips
    /// back to false and starts a fresh poll cycle (so the UI sees
    /// immediate data, not a 1s-delay until the next tick).
    public func resumePolling() {
        isPaused = false
        startPolling()
    }

    public func refresh() async {
        do {
            let report = try await client.monitoringReport()
            let effectiveness = (try? await client.poolEffectiveness())
            let now = Date()
            processes = report.asProcessSummaries()
            health = report.asHealthSnapshot()
            poolStatus = report.pool
            statusSnapshot = report.status
            poolEffectiveness = effectiveness
            isConnected = true
            lastError = nil

            // Rolling-window maintenance for the dashboard's PR 6 panels.
            // We capture the timestamp at the moment the snapshot lands so the
            // sparkline x-axis is wall-clock-correct (not the sidecar's
            // `report.timestamp`, which is the moment the sidecar produced the
            // snapshot and can lag the poll by a few hundred ms).
            hostWatchHistory.append(HostWatchSample(timestamp: now, host: report.host_watch))
            if hostWatchHistory.count > Self.hostWatchHistoryCap {
                hostWatchHistory.removeFirst(hostWatchHistory.count - Self.hostWatchHistoryCap)
            }
            gateDecisionHistory.append(GateDecisionSample(timestamp: now, gate: report.gate))
            if gateDecisionHistory.count > Self.gateDecisionHistoryCap {
                gateDecisionHistory.removeFirst(gateDecisionHistory.count - Self.gateDecisionHistoryCap)
            }
            // Pool effectiveness rolling window — captured separately because
            // effectiveness is its own IPC round-trip. Cheap; same cap as
            // host watch.
            if let eff = effectiveness {
                poolEffectivenessHistory.append(eff)
                if poolEffectivenessHistory.count > Self.poolEffectivenessHistoryCap {
                    poolEffectivenessHistory.removeFirst(
                        poolEffectivenessHistory.count - Self.poolEffectivenessHistoryCap
                    )
                }
            }

            // Per-agent RSS history ring buffer (PR 5). Each agent's RSS
            // is appended to its own per-PID array. New PIDs get a fresh
            // array; PIDs that have left the agents list have their entry
            // evicted from the agentRSSHistory map (and their cmdline
            // cache entry too).
            let livePIDs = Set(report.status.agents.map { $0.pid })
            for agent in report.status.agents {
                var series = agentRSSHistory[agent.pid] ?? []
                series.append(agent.mem_rss_bytes)
                if series.count > Self.agentRSSHistoryCap {
                    series.removeFirst(series.count - Self.agentRSSHistoryCap)
                }
                agentRSSHistory[agent.pid] = series
            }
            let stalePIDs = Set(agentRSSHistory.keys).subtracting(livePIDs)
            for pid in stalePIDs {
                agentRSSHistory.removeValue(forKey: pid)
                cmdlineCache.removeValue(forKey: pid)
            }

            // Fleet aggregates (PR 2b of processes-page expansion).
            // Total RSS = sum of all processes' memory_mb. Used RSS =
            // total minus idle-row-excluded; for now we just use total.
            let totalRSS = report.processes.reduce(UInt64(0)) { $0 + UInt64($1.memory_mb) * 1024 * 1024 }
            let avgCPU: Float = report.processes.isEmpty
                ? 0
                : report.processes.reduce(Float(0)) { $0 + $1.cpu_percent }
                  / Float(report.processes.count)
            let sample = FleetSample(
                timestamp: now,
                totalProcesses: report.processes.count,
                totalMemoryMB: report.processes.reduce(UInt64(0)) { $0 + UInt64($1.memory_mb) },
                usedMemoryMB: report.processes.reduce(UInt64(0)) { $0 + UInt64($1.memory_mb) },
                cpuAvgPercent: avgCPU,
                poolHealthy: report.pool.healthy
            )
            fleetHistory.append(sample)
            if fleetHistory.count > Self.fleetHistoryCap {
                fleetHistory.removeFirst(fleetHistory.count - Self.fleetHistoryCap)
            }

            NotificationCenter.default.post(
                name: .sharecliHealthChanged,
                object: health
            )
        } catch {
            isConnected = false
            lastError = error.localizedDescription
            poolStatus = nil
            statusSnapshot = nil
            // On IPC failure we keep the rolling windows so the last-known
            // sparklines remain visible. Clearing them would flash empty.
            NotificationCenter.default.post(
                name: .sharecliHealthChanged,
                object: nil
            )
        }
    }

    public func kill(pid: UInt32) async {
        do {
            try await client.kill(pid: pid)
            await refresh()
        } catch {
            lastError = "kill \(pid): \(error.localizedDescription)"
        }
    }

    public func killAll() async {
        do {
            try await client.killAll()
            await refresh()
        } catch {
            lastError = "kill_all: \(error.localizedDescription)"
        }
    }

    /// Append a spawn attempt to `spawnHistory` (capped at spawnHistoryCap),
    /// persist to disk, and publish via spawnHistoryChanged.
    public func recordSpawn(_ entry: SpawnHistoryEntry) {
        spawnHistory.append(entry)
        if spawnHistory.count > Self.spawnHistoryCap {
            spawnHistory.removeFirst(spawnHistory.count - Self.spawnHistoryCap)
        }
        NotificationCenter.default.post(name: .sharecliSpawnHistoryChanged, object: nil)
        persistSpawnHistory()
    }

    /// Persist `spawnHistory` to `~/Library/Application Support/sharecli/spawn_history.json`.
    /// Errors are surfaced via `lastError` (non-fatal).
    private func persistSpawnHistory() {
        do {
            let url = try spawnHistoryURL()
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(spawnHistory)
            try data.write(to: url, options: [.atomic])
        } catch {
            lastError = "spawn_history.persist: \(error.localizedDescription)"
        }
    }

    /// Load any persisted spawn history from disk. Called from init.
    private func loadSpawnHistory() {
        do {
            let url = try spawnHistoryURL()
            guard FileManager.default.fileExists(atPath: url.path) else { return }
            let data = try Data(contentsOf: url)
            let decoded = try JSONDecoder().decode([SpawnHistoryEntry].self, from: data)
            // Trim to cap in case cap shrunk between versions.
            self.spawnHistory = Array(decoded.suffix(Self.spawnHistoryCap))
        } catch {
            // Non-fatal: a corrupt file just falls back to empty history.
            // Don't surface via lastError on startup — would be noise.
        }
    }

    private func spawnHistoryURL() throws -> URL {
        let support = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dir = support.appendingPathComponent("sharecli", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("spawn_history.json")
    }

    /// Fetch the cmdline for `pid` if not already cached. Returns the
    /// cached value on subsequent calls. Returns `nil` if the sidecar
    /// couldn't read the cmdline (process gone, non-Linux platform).
    public func fetchCmdlineIfNeeded(pid: UInt32) async -> ProcessCmdline? {
        if let cached = cmdlineCache[pid] { return cached }
        do {
            let snap = try await client.fetchCmdline(pid: pid)
            if let snap = snap {
                cmdlineCache[pid] = snap
            }
            return snap
        } catch {
            lastError = "process.cmdline \(pid): \(error.localizedDescription)"
            return nil
        }
    }

    // MARK: - Config

    public func setConfig(key: String, value: AnyCodable) async {
        do {
            try await client.setConfig(key: key, value: value)
        } catch {
            lastError = "config.set \(key): \(error.localizedDescription)"
        }
    }

    /// Fetch the live config from the sidecar (PR 7 of dashboard expansion plan).
    /// Returns the raw JSON `Data` — the page parses it for the form + JSON preview.
    public func getConfig() async -> Data? {
        do {
            return try await client.getConfig()
        } catch {
            lastError = "config.get: \(error.localizedDescription)"
            return nil
        }
    }
}

public extension Notification.Name {
    /// Posted on the main thread whenever AppState.refresh() completes
    /// (carries a HealthSnapshot? as object — nil when IPC disconnected).
    static let sharecliHealthChanged = Notification.Name("sharecliHealthChanged")

    /// Posted whenever AppState.recordSpawn(_:) appends to `spawnHistory`.
    /// Used by SpawnView to refresh its recent-attempts panel.
    static let sharecliSpawnHistoryChanged = Notification.Name("sharecliSpawnHistoryChanged")

    /// Posted whenever AppState captures a new FleetChange (processes
    /// added/removed since the previous refresh). Carries a FleetChange
    /// as the notification object.
    static let sharecliFleetChanged = Notification.Name("sharecliFleetChanged")
}

/// Describes the delta between two consecutive fleet snapshots. Computed by
/// `AppState.refresh()` after every monitoring refresh and surfaced via
/// the Processes → Diff subpage. All fields default to empty so partial
/// changes (e.g. only additions) are easy to express.
public struct FleetChange: Equatable, Hashable {
    public let timestamp: Date
    public let added: [ProcessSummary]
    public let removed: [ProcessSummary]
    public let stayed: Int   // count of processes present in both snapshots

    public init(timestamp: Date = Date(),
                added: [ProcessSummary] = [],
                removed: [ProcessSummary] = [],
                stayed: Int = 0) {
        self.timestamp = timestamp
        self.added = added
        self.removed = removed
        self.stayed = stayed
    }

    public var isEmpty: Bool { added.isEmpty && removed.isEmpty }
    public var totalChanged: Int { added.count + removed.count }
}
