/// IPCClient.swift — Unix socket NDJSON-RPC client for the sharecli IPC server.
///
/// All calls are async; the caller awaits on a background Task.
/// Thread-safety: each call creates its own socket connection (stateless from
/// the Swift side — the Rust server handles concurrent connections).

import Foundation
import Network

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

public struct IPCRequest: Encodable {
    public let id: Int
    public let method: String
    public let params: [String: AnyCodable]

    public init(id: Int, method: String, params: [String: AnyCodable] = [:]) {
        self.id = id
        self.method = method
        self.params = params
    }
}

public struct IPCResponse<T: Decodable>: Decodable {
    public let id: Int
    public let result: T?
    public let error: String?
}

// ---------------------------------------------------------------------------
// Data models
// ---------------------------------------------------------------------------

public struct ProcessSummary: Identifiable, Decodable, Hashable, Encodable {
    public var id: UInt32 { pid }
    public let pid: UInt32
    public let name: String
    public let cmd: [String]
    public let memory_mb: UInt64
    public let project: String?
    public let harness: String?
    public let start_time: UInt64
    /// CPU utilization percentage (0..100 * num_cores). Defaults to 0 when
    /// the sidecar is older than the cpu_percent IPC extension, or on the
    /// very first sysinfo sample for a freshly-spawned process. Backed by
    /// explicit CodingKeys + init(from:) so older sidecars (missing the
    /// `cpu_percent` key) still decode cleanly.
    public let cpu_percent: Float
    /// Parent PID. nil when unavailable (kernel threads, sandboxed).
    public let ppid: UInt32?
    /// Current working directory (best-effort).
    public let cwd: String?
    /// Number of env vars on the process.
    public let env_count: UInt32
    /// Stable process state enum ("idle" / "run" / "sleep" / "stop" /
    /// "zombie" / "tracing" / "dead" / "unknown").
    public let state: String
    /// Disk read bytes (Linux-only).
    public let disk_read_bytes: UInt64?
    /// Disk write bytes (Linux-only).
    public let disk_write_bytes: UInt64?
    /// Open FD count (computed via `lsof`).
    public let fd_count: UInt32?

    private enum CodingKeys: String, CodingKey {
        case pid, name, cmd, memory_mb, project, harness, start_time
        case cpu_percent, ppid, cwd, env_count, state
        case disk_read_bytes, disk_write_bytes, fd_count
    }

    public init(
        pid: UInt32,
        name: String,
        cmd: [String],
        memory_mb: UInt64,
        project: String? = nil,
        harness: String? = nil,
        start_time: UInt64 = 0,
        cpu_percent: Float = 0,
        ppid: UInt32? = nil,
        cwd: String? = nil,
        env_count: UInt32 = 0,
        state: String = "unknown",
        disk_read_bytes: UInt64? = nil,
        disk_write_bytes: UInt64? = nil,
        fd_count: UInt32? = nil
    ) {
        self.pid = pid
        self.name = name
        self.cmd = cmd
        self.memory_mb = memory_mb
        self.project = project
        self.harness = harness
        self.start_time = start_time
        self.cpu_percent = cpu_percent
        self.ppid = ppid
        self.cwd = cwd
        self.env_count = env_count
        self.state = state
        self.disk_read_bytes = disk_read_bytes
        self.disk_write_bytes = disk_write_bytes
        self.fd_count = fd_count
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        pid        = try c.decode(UInt32.self, forKey: .pid)
        name       = try c.decode(String.self, forKey: .name)
        cmd        = try c.decode([String].self, forKey: .cmd)
        memory_mb  = try c.decode(UInt64.self, forKey: .memory_mb)
        project    = try c.decodeIfPresent(String.self, forKey: .project)
        harness    = try c.decodeIfPresent(String.self, forKey: .harness)
        start_time = try c.decode(UInt64.self, forKey: .start_time)
        cpu_percent = (try? c.decode(Float.self, forKey: .cpu_percent)) ?? 0
        ppid        = try c.decodeIfPresent(UInt32.self, forKey: .ppid)
        cwd         = try c.decodeIfPresent(String.self, forKey: .cwd)
        env_count   = (try? c.decode(UInt32.self, forKey: .env_count)) ?? 0
        state       = (try? c.decode(String.self, forKey: .state)) ?? "unknown"
        disk_read_bytes  = try c.decodeIfPresent(UInt64.self, forKey: .disk_read_bytes)
        disk_write_bytes = try c.decodeIfPresent(UInt64.self, forKey: .disk_write_bytes)
        fd_count    = try c.decodeIfPresent(UInt32.self, forKey: .fd_count)
    }

    // MARK: - Non-optional shadow fields (for sortable Table columns)

    public var fdCountValue: UInt32 { fd_count ?? 0 }
    public var ioReadValue: UInt64 { disk_read_bytes ?? 0 }
    public var ioWriteValue: UInt64 { disk_write_bytes ?? 0 }
    public var ppidValue: UInt32 { ppid ?? 0 }
    public var stateValue: String { state }
}

public struct GateStatusSnapshot: Decodable, Hashable {
    public let thermal_pressure: String
    public let detected_agents: Int
    public let agent_total_rss_bytes: UInt64
    public let agent_contention: String
    public let gate_decision: String
}

public struct HostResourceWatchJson: Decodable, Hashable {
    public let fd_count: UInt64
    public let net_rx_bytes: UInt64
    public let net_tx_bytes: UInt64
    public let mem_rss_bytes: UInt64
    public let load_1m: Double
}

public struct HealthSnapshot: Decodable {
    public let managed_processes: Int
    public let used_memory_mb: UInt64
    public let total_memory_mb: UInt64
    public let healthy: Bool
    public let gate: GateStatusSnapshot
    public let host_watch: HostResourceWatchJson
}

public struct MonitoringProcessEntry: Decodable, Hashable {
    public let pid: UInt32
    public let name: String
    public let memory_mb: UInt64
    public let project: String?
    public let harness: String?
    /// Unix timestamp (seconds) the process started. 0 when the sidecar
    /// couldn't determine start_time or when running against older sidecars.
    public let start_time: UInt64
    /// CPU utilization percentage (0..100 * num_cores). 0 when the sidecar
    /// is older than the cpu_percent IPC extension, or on the very first
    /// sysinfo sample for a freshly-spawned process.
    public let cpu_percent: Float
    /// Parent PID for tree view. `nil` for kernel threads or when the
    /// platform couldn't resolve one (e.g. macOS sandboxed process).
    public let ppid: UInt32?
    /// Current working directory (best-effort). Empty on platforms where
    /// the kernel doesn't expose it.
    public let cwd: String?
    /// Count of environment variables on the process. Cross-platform
    /// (computed from `sysinfo::Process::environ().len()`).
    public let env_count: UInt32
    /// Process state mapped through `ProcState` for stable serialization.
    public let state: String
    /// Total bytes read from disk. Linux-only (via
    /// `disk_usage().total_read_bytes`); `nil` on macOS/Windows.
    public let disk_read_bytes: UInt64?
    /// Total bytes written to disk. Linux-only.
    public let disk_write_bytes: UInt64?
    /// Open file descriptor count. Computed cross-platform via
    /// `lsof -p <pid>` (macOS/Linux); `nil` if lsof is unavailable or the
    /// sidecar lacks permission.
    public let fd_count: UInt32?
}

public struct MonitoringReportSnapshot: Decodable {
    public let timestamp: UInt64
    public let total_processes: Int
    public let used_memory_mb: UInt64
    public let total_memory_mb: UInt64
    public let processes: [MonitoringProcessEntry]
    public let gate: GateStatusSnapshot
    public let host_watch: HostResourceWatchJson
    public let pool: PoolSnapshot
    public let status: StatusSnapshot

    /// Map fleet monitoring snapshot → tray health fields (parity with `health.status`).
    public func asHealthSnapshot() -> HealthSnapshot {
        HealthSnapshot(
            managed_processes: total_processes,
            used_memory_mb: used_memory_mb,
            total_memory_mb: total_memory_mb,
            healthy: used_memory_mb < total_memory_mb / 2,
            gate: gate,
            host_watch: host_watch
        )
    }

    /// Map fleet monitoring processes → tray process rows (parity with `process.list`).
    public func asProcessSummaries() -> [ProcessSummary] {
        processes.map { entry in
            ProcessSummary(
                pid: entry.pid,
                name: entry.name,
                cmd: [],
                memory_mb: entry.memory_mb,
                project: entry.project,
                harness: entry.harness,
                start_time: entry.start_time,
                cpu_percent: entry.cpu_percent,
                ppid: entry.ppid,
                cwd: entry.cwd,
                env_count: entry.env_count,
                state: entry.state,
                disk_read_bytes: entry.disk_read_bytes,
                disk_write_bytes: entry.disk_write_bytes,
                fd_count: entry.fd_count
            )
        }
    }
}

public struct AgentProcRow: Decodable, Hashable, Identifiable {
    public var id: UInt32 { pid }
    public let pid: UInt32
    public let family: String
    public let comm: String
    public let state: String
    public let mem_rss_bytes: UInt64
    public let mem_rss: String
    public let fd_count: UInt64?
}

/// IPC `pool.status` envelope (FR-007 / AC-007.67, tray wire AC-007.68).
public struct PoolSnapshot: Decodable {
    public let node_total: Int
    public let node_idle: Int
    public let bun_total: Int
    public let bun_idle: Int
    public let max_per_type: Int
    public let healthy: Bool
    public let issues: [String]
    public let gate: GateStatusSnapshot
    public let host_watch: HostResourceWatchJson
}

/// IPC `status.snapshot` envelope (FR-007 / AC-007.67, tray wire AC-007.68).
public struct StatusSnapshot: Decodable {
    public let total_processes: Int
    public let agents: [AgentProcRow]
    public let scanned: Int
    public let watched: Int
    public let gate: GateStatusSnapshot
    public let host_watch: HostResourceWatchJson
    /// Filesystem path to the sharecli log file (PR 8 of dashboard expansion).
    /// The Swift tray reads this file directly with tail -F semantics — no
    /// separate log.tail IPC needed. Tolerant of older sidecar builds that
    /// don't yet emit log_location (yields a nil live_log_path).
    public let log_location: String?
    /// Convenience accessor — `FileManager.tilde`-expanded path or nil.
    public var live_log_path: URL? {
        guard let raw = log_location, !raw.isEmpty else { return nil }
        let fm = FileManager.default
        let expanded = (raw as NSString).expandingTildeInPath
        let url = URL(fileURLWithPath: expanded)
        return fm.fileExists(atPath: url.path) ? url : nil
    }
}

/// IPC `pool.effectiveness` envelope (PR 4 of dashboard expansion plan).
///
/// Aggregates Hypervisor coalesce cache + SlotQueue counters from
/// `sharecli_fleet` global atomics. Cheap, snapshot-style — no
/// per-process scanning.
public struct CoalesceMetersSnapshot: Decodable, Hashable {
    public let hits: UInt64
    public let misses: UInt64
    public let nocache_runs: UInt64

    public var hitRatePct: Double {
        let total = hits + misses
        guard total > 0 else { return 0 }
        return Double(hits) / Double(total) * 100.0
    }
}

public struct SlotQueueMetersSnapshot: Decodable, Hashable {
    public let acquires: UInt64
    public let waits: UInt64
    public let timeouts: UInt64
}

public struct PoolEffectivenessSnapshot: Decodable, Hashable {
    public let coalesce: CoalesceMetersSnapshot
    public let slot_queue: SlotQueueMetersSnapshot
    public let sampled_at: UInt64
}

/// IPC `process.cmdline` envelope (PR 5 of dashboard expansion plan).
///
/// Returns the full command-line for a given PID, plus the parsed argv
/// (whitespace-split, naive). `cmdline` is the raw
/// `/proc/<pid>/cmdline` buffer (NUL-separated, '\n'-joined) so the
/// tray can render it verbatim. `argv` is the whitespace-split array
/// for table-friendly display.
///
/// On macOS the sidecar returns `cmdline: ""` (no `/proc` filesystem).
public struct ProcessCmdline: Decodable, Hashable {
    public let pid: UInt32
    public let cmdline: String
    public let argv: [String]
}

// MARK: - Per-process detail IPC (process.fdcount, process.io)

public struct ProcessFdcountResult: Decodable, Hashable {
    public let pid: UInt32
    public let fd_count: UInt32?
    public let sampled_at: UInt64
    public let note: String?
}

public struct ProcessIoResult: Decodable, Hashable {
    public let pid: UInt32
    public let disk_read_bytes: UInt64?
    public let disk_write_bytes: UInt64?
    public let sampled_at: UInt64
    public let note: String?
}

// MARK: - Tree view (process.tree)

public struct ProcessTreeNode: Decodable, Hashable, Identifiable {
    public var id: UInt32 { pid }
    public let pid: UInt32
    public let name: String
    public let children: [ProcessTreeNode]
}

// MARK: - Spawn (process.spawn)

public struct ProcessSpawnPayload: Encodable {
    public let name: String
    public let command: String
    public let args: [String]
    public let project: String?
    public let harness: String?

    public init(name: String, command: String, args: [String], project: String?, harness: String?) {
        self.name = name
        self.command = command
        self.args = args
        self.project = project
        self.harness = harness
    }
}

public struct ProcessSpawnResult: Decodable, Hashable {
    public let pid: UInt32
    public let success: Bool
    public let error: String?
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

public actor IPCClient {
    private let _socketPath: String
    /// Public read-only accessor for the Unix socket path this client
    /// connects to. Used by the preferences sheet + status bar tooltip.
    public var socketPath: String { _socketPath }
    private var nextId: Int = 1

    public init(socketPath: String) {
        self._socketPath = socketPath
    }

    public static func defaultClient() -> IPCClient {
        let env = ProcessInfo.processInfo.environment["SHARECLI_IPC_SOCK"]
        let defaultPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/sharecli/ipc.sock")
            .path
        return IPCClient(socketPath: env ?? defaultPath)
    }

    /// Free helper for views that want the default Unix socket path
    /// without holding a client reference. Returns the tilde-expanded
    /// path the tray would use to connect to the sidecar.
    public static func defaultSocketPath() -> String {
        let env = ProcessInfo.processInfo.environment["SHARECLI_IPC_SOCK"]
        let defaultPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/sharecli/ipc.sock")
            .path
        let raw = env ?? defaultPath
        return (raw as NSString).expandingTildeInPath
    }

    private func nextRequestId() -> Int {
        defer { nextId += 1 }
        return nextId
    }

    // MARK: - Public API

    public func listProcesses() async throws -> [ProcessSummary] {
        let resp: IPCResponse<[ProcessSummary]> = try await call(
            method: "process.list", params: [:]
        )
        return resp.result ?? []
    }

    public func kill(pid: UInt32) async throws {
        let _: IPCResponse<Bool> = try await call(
            method: "process.kill", params: ["pid": .uint(pid)]
        )
    }

    public func killAll() async throws {
        let _: IPCResponse<Bool> = try await call(
            method: "process.kill_all", params: [:]
        )
    }

    public func health() async throws -> HealthSnapshot {
        let resp: IPCResponse<HealthSnapshot> = try await call(
            method: "health.status", params: [:]
        )
        guard let snap = resp.result else {
            throw IPCError.nilResult("health.status")
        }
        return snap
    }

    public func monitoringReport() async throws -> MonitoringReportSnapshot {
        let resp: IPCResponse<MonitoringReportSnapshot> = try await call(
            method: "monitoring.report", params: [:]
        )
        guard let snap = resp.result else {
            throw IPCError.nilResult("monitoring.report")
        }
        return snap
    }

    public func poolStatus() async throws -> PoolSnapshot {
        let resp: IPCResponse<PoolSnapshot> = try await call(
            method: "pool.status", params: [:]
        )
        guard let snap = resp.result else {
            throw IPCError.nilResult("pool.status")
        }
        return snap
    }

    public func poolEffectiveness() async throws -> PoolEffectivenessSnapshot {
        let resp: IPCResponse<PoolEffectivenessSnapshot> = try await call(
            method: "pool.effectiveness", params: [:]
        )
        guard let snap = resp.result else {
            throw IPCError.nilResult("pool.effectiveness")
        }
        return snap
    }

    public func statusSnapshot() async throws -> StatusSnapshot {
        let resp: IPCResponse<StatusSnapshot> = try await call(
            method: "status.snapshot", params: [:]
        )
        guard let snap = resp.result else {
            throw IPCError.nilResult("status.snapshot")
        }
        return snap
    }

    /// Fetch the command-line + argv for a specific PID.
    /// Returns `nil` (not throws) if the sidecar returns an empty
    /// cmdline (process gone, or non-Linux platform).
    public func fetchCmdline(pid: UInt32) async throws -> ProcessCmdline? {
        let resp: IPCResponse<ProcessCmdline> = try await call(
            method: "process.cmdline", params: ["pid": .uint(pid)]
        )
        guard let snap = resp.result else { return nil }
        return snap.cmdline.isEmpty ? nil : snap
    }

    /// Per-process FD count (cross-platform via `lsof`).
    /// Returns `fd_count: nil` + a `note` if the sidecar couldn't read it.
    public func fetchFdcount(pid: UInt32) async throws -> ProcessFdcountResult {
        let resp: IPCResponse<ProcessFdcountResult> = try await call(
            method: "process.fdcount", params: ["pid": .uint(pid)]
        )
        guard let snap = resp.result else { throw IPCError.nilResult("process.fdcount") }
        return snap
    }

    /// Per-process disk I/O bytes (Linux-only).
    /// Returns `disk_read_bytes: nil` + a `note` if not available.
    public func fetchIo(pid: UInt32) async throws -> ProcessIoResult {
        let resp: IPCResponse<ProcessIoResult> = try await call(
            method: "process.io", params: ["pid": .uint(pid)]
        )
        guard let snap = resp.result else { throw IPCError.nilResult("process.io") }
        return snap
    }

    /// Fetch a flat tree of the process fleet (parent → children).
    public func fetchProcessTree() async throws -> ProcessTreeNode? {
        let resp: IPCResponse<ProcessTreeNode> = try await call(
            method: "process.tree", params: [:]
        )
        return resp.result
    }

    /// Spawn a new process in the pool. The sidecar returns the assigned PID
    /// on success, or `success: false` with `error` populated.
    public func spawn(payload: ProcessSpawnPayload) async throws -> ProcessSpawnResult {
        let resp: IPCResponse<ProcessSpawnResult> = try await call(
            method: "process.spawn",
            params: [
                "name": .string(payload.name),
                "command": .string(payload.command),
                "args": .array(payload.args.map { .string($0) }),
                "project": payload.project.map(AnyCodable.string) ?? .null,
                "harness": payload.harness.map(AnyCodable.string) ?? .null
            ]
        )
        guard let snap = resp.result else { throw IPCError.nilResult("process.spawn") }
        return snap
    }

    public func getConfig() async throws -> Data {
        let resp: IPCResponse<AnyCodable> = try await call(
            method: "config.get", params: [:]
        )
        guard let raw = resp.result else { throw IPCError.nilResult("config.get") }
        return try JSONEncoder().encode(raw)
    }

    public func setConfig(key: String, value: AnyCodable) async throws {
        let _: IPCResponse<Bool> = try await call(
            method: "config.set", params: ["key": .string(key), "value": value]
        )
    }

    // MARK: - Transport

    private func call<T: Decodable>(
        method: String,
        params: [String: AnyCodable]
    ) async throws -> IPCResponse<T> {
        let id = nextRequestId()
        let req = IPCRequest(id: id, method: method, params: params)
        var payload = try JSONEncoder().encode(req)
        payload.append(contentsOf: [UInt8(ascii: "\n")])

        let sock = socketPath
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                do {
                    let fd = try Self.openUnixSocket(path: sock)
                    defer { Darwin.close(fd) }

                    // Write request
                    try payload.withUnsafeBytes { buf in
                        var written = 0
                        while written < buf.count {
                            let n = Darwin.write(fd, buf.baseAddress!.advanced(by: written), buf.count - written)
                            guard n > 0 else { throw IPCError.writeFailed }
                            written += n
                        }
                    }

                    // Read until newline
                    var response = Data()
                    var byte = UInt8(0)
                    while true {
                        let n = Darwin.read(fd, &byte, 1)
                        guard n > 0 else { throw IPCError.readFailed }
                        if byte == UInt8(ascii: "\n") { break }
                        response.append(byte)
                    }

                    let decoded = try JSONDecoder().decode(IPCResponse<T>.self, from: response)

                    // The daemon reports refusals (for example a rejected
                    // config patch) in `error` with a null result. Surface it
                    // as a thrown error, otherwise callers treat the refusal as
                    // success and the tray shows a false "Applied" toast for a
                    // write that never happened.
                    if let message = decoded.error {
                        throw IPCError.server(message)
                    }

                    continuation.resume(returning: decoded)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func openUnixSocket(path: String) throws -> Int32 {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw IPCError.socketCreate }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathCap = MemoryLayout.size(ofValue: addr.sun_path)
        path.withCString { cstr in
            withUnsafeMutableBytes(of: &addr.sun_path) { raw in
                guard let base = raw.baseAddress else { return }
                let dest = base.assumingMemoryBound(to: CChar.self)
                let n = min(pathCap, raw.count)
                memset(dest, 0, n)
                strncpy(dest, cstr, n > 0 ? n - 1 : 0)
            }
        }

        let connectResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sap in
                connect(fd, sap, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }

        guard connectResult == 0 else {
            Darwin.close(fd)
            throw IPCError.connectFailed(path)
        }
        return fd
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

public enum IPCError: LocalizedError {
    case socketCreate
    case connectFailed(String)
    case writeFailed
    case readFailed
    case nilResult(String)
    /// The daemon answered with a populated `error` field.
    case server(String)

    public var errorDescription: String? {
        switch self {
        case .socketCreate: return "Failed to create Unix socket"
        case .connectFailed(let p): return "Could not connect to sharecli-ipc at \(p)"
        case .writeFailed: return "Socket write failed"
        case .readFailed: return "Socket read failed"
        case .nilResult(let m): return "Nil result from \(m)"
        case .server(let m): return m
        }
    }
}

// ---------------------------------------------------------------------------
// AnyCodable — lightweight type-erased JSON value
// ---------------------------------------------------------------------------

public enum AnyCodable: Codable {
    case string(String)
    case int(Int)
    case uint(UInt32)
    case double(Double)
    case bool(Bool)
    case array([AnyCodable])
    case object([String: AnyCodable])
    case null

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if let v = try? c.decode(String.self) { self = .string(v) }
        else if let v = try? c.decode(Int.self) { self = .int(v) }
        else if let v = try? c.decode(Double.self) { self = .double(v) }
        else if let v = try? c.decode(Bool.self) { self = .bool(v) }
        else if let v = try? c.decode([AnyCodable].self) { self = .array(v) }
        else if let v = try? c.decode([String: AnyCodable].self) { self = .object(v) }
        else { self = .null }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .string(let v): try c.encode(v)
        case .int(let v): try c.encode(v)
        case .uint(let v): try c.encode(v)
        case .double(let v): try c.encode(v)
        case .bool(let v): try c.encode(v)
        case .array(let v): try c.encode(v)
        case .object(let v): try c.encode(v)
        case .null: try c.encodeNil()
        }
    }
}
