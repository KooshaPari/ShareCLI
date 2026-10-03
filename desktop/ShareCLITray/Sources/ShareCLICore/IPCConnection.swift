/// IPCConnection.swift — one long-lived NDJSON connection to the sidecar.
///
/// `IPCClient` previously opened a socket per call, wrote one request, read the
/// reply, and closed. This keeps a single connection open for the life of the
/// client and routes replies by request id, so concurrent calls stay
/// concurrent instead of being serialised behind one socket.
///
/// Two audit items are closed here:
/// * `FINDINGS.md:61` `[HIGH]` — the old read loop was
///   `while true { Darwin.read(fd, &byte, 1) }`, one syscall per byte and 10⁵+
///   syscalls per 1 Hz `monitoring.report`. The reader now drains
///   `readChunkBytes` at a time.
/// * `PLAN.md:164` — single long-lived connection with an id→continuation map
///   and per-request cancellation.
///
/// ## Concurrency model
///
/// One **serial** `DispatchQueue` owns the file descriptor. Both the reader and
/// every write run on it, so there is exactly one close path and no operation
/// can be inside a syscall on an fd another path is closing. `pending` is only
/// ever touched on that queue, so it needs no lock.
///
/// The fd is non-blocking and is drained by a timer rather than by a
/// `DispatchSource` bound to the fd: a timer cannot be signalled against a
/// descriptor that has already been closed, which is the classic crash in
/// fd-backed sources. The cost is one `read(2)` returning `EAGAIN` per tick
/// (10 ms) while the tray is idle; the benefit is that a sidecar restart is
/// noticed promptly, so the next call reconnects instead of writing into a
/// dead socket — the property the old per-call connection had for free.
///
/// Every registered continuation is guaranteed to resume: each `send` schedules
/// its own timeout on the same queue and that timer holds the connection alive
/// until it fires, so a continuation can never be dropped along with its owner.
import Foundation

final class IPCConnection: @unchecked Sendable {
    /// Bytes pulled from the socket by one `read(2)`. Chosen to be far larger
    /// than any reply the sidecar sends in practice, so a reply costs one
    /// syscall instead of one per byte.
    static let readChunkBytes = 64 * 1024

    /// Reader tick. Also the upper bound on how long a reply, or a peer
    /// hang-up, can go unnoticed before the connection state is corrected.
    static let readerInterval: DispatchTimeInterval = .milliseconds(10)

    /// Bound on bytes held for one reply. The server caps a *request* frame at
    /// `framing::MAX_REQUEST_LINE_BYTES`; this is the mirror on the client so a
    /// peer that never sends `\n` cannot grow the tray either.
    static let maxBufferedBytes = 1024 * 1024

    private let queue = DispatchQueue(label: "sharecli.ipc.connection")
    private let socketPath: String
    private let timeout: TimeInterval

    // All three are queue-owned.
    private var fd: Int32 = -1
    private var reader: DispatchSourceTimer?
    private var pending: [Int: CheckedContinuation<Data, Error>] = [:]
    private var buffer = Data()

    init(socketPath: String, timeout: TimeInterval) {
        self.socketPath = socketPath
        self.timeout = timeout
    }

    deinit {
        // A queued handler resolves a weak `self`, so it cannot be about to run
        // if we are here: deinit proves no handler holds this instance.
        reader?.setEventHandler {}
        reader?.cancel()
        if fd >= 0 {
            Darwin.close(fd)
        }
    }

    /// Open the connection now.
    ///
    /// Throws `.alreadyConnected` when one is already live — the audit's
    /// "second connection attempt rejected" (`PLAN.md:166`). Normal traffic
    /// goes through `send`, which reuses the live connection instead.
    func connect() throws {
        try queue.sync { try openLocked() }
    }

    /// Write one request frame and await its reply, routed by `id`.
    ///
    /// The reply is the raw line, not a decoded value: `pending` maps to `Data`
    /// so one map can serve every `T` a caller asks `IPCClient` to decode.
    func send(id: Int, frame: Data) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    try self.ensureConnectedLocked()
                } catch {
                    continuation.resume(throwing: error)
                    return
                }

                self.pending[id] = continuation
                self.scheduleTimeoutLocked(id: id)

                do {
                    try self.writeLocked(frame)
                } catch {
                    // Only this request fails: the others keep their own
                    // deadlines, and a late reply finds no entry and is dropped.
                    self.pending.removeValue(forKey: id)?.resume(throwing: error)
                }
            }
        }
    }

    // MARK: - Queue-owned connection state

    private func ensureConnectedLocked() throws {
        if fd < 0 {
            try openLocked()
        }
    }

    private func openLocked() throws {
        guard fd < 0 else { throw IPCError.alreadyConnected }

        let newFd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard newFd >= 0 else { throw IPCError.socketCreate }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathCap = MemoryLayout.size(ofValue: addr.sun_path)
        socketPath.withCString { cstr in
            withUnsafeMutableBytes(of: &addr.sun_path) { raw in
                guard let base = raw.baseAddress else { return }
                let dest = base.assumingMemoryBound(to: CChar.self)
                let n = min(pathCap, raw.count)
                memset(dest, 0, n)
                strncpy(dest, cstr, n > 0 ? n - 1 : 0)
            }
        }

        let connected = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sap in
                // Qualified: inside this type, bare `connect` resolves to our
                // instance method of the same name.
                Darwin.connect(newFd, sap, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            Darwin.close(newFd)
            throw IPCError.connectFailed(socketPath)
        }

        // macOS raises SIGPIPE on write to a peer that has gone away, which
        // would terminate the tray. The old per-call path had the same gap; a
        // connection that now outlives sidecar restarts hits it often enough
        // that it has to be closed here.
        var nosigpipe: Int32 = 1
        setsockopt(
            newFd, SOL_SOCKET, SO_NOSIGPIPE, &nosigpipe, socklen_t(MemoryLayout<Int32>.size)
        )

        let flags = fcntl(newFd, F_GETFL, 0)
        guard flags >= 0, fcntl(newFd, F_SETFL, flags | O_NONBLOCK) >= 0 else {
            Darwin.close(newFd)
            throw IPCError.connectFailed(socketPath)
        }

        fd = newFd
        buffer.removeAll(keepingCapacity: false)
        startReaderLocked()
    }

    private func startReaderLocked() {
        reader?.setEventHandler {}
        reader?.cancel()

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + Self.readerInterval, repeating: Self.readerInterval)
        timer.setEventHandler { [weak self] in
            self?.drainLocked()
        }
        timer.resume()
        reader = timer
    }

    private func closeLocked() {
        reader?.setEventHandler {}
        reader?.cancel()
        reader = nil
        if fd >= 0 {
            Darwin.close(fd)
            fd = -1
        }
        buffer.removeAll(keepingCapacity: false)
    }

    // MARK: - Read side

    private func drainLocked() {
        guard fd >= 0 else { return }

        var chunk = [UInt8](repeating: 0, count: Self.readChunkBytes)
        while fd >= 0 {
            let n = chunk.withUnsafeMutableBytes { raw -> Int in
                guard let base = raw.baseAddress else { return -1 }
                return Darwin.read(fd, base, raw.count)
            }

            if n > 0 {
                buffer.append(contentsOf: chunk[0..<n])
                if buffer.count > Self.maxBufferedBytes {
                    failAllLocked(IPCError.readFailed)
                    closeLocked()
                    return
                }
                continue
            }
            if n == 0 {
                // Peer hang-up: sidecar restarted or exited.
                failAllLocked(IPCError.readFailed)
                closeLocked()
                return
            }
            if errno == EINTR { continue }
            if errno == EAGAIN || errno == EWOULDBLOCK { break }
            failAllLocked(IPCError.readFailed)
            closeLocked()
            return
        }

        splitLinesLocked()
    }

    private func splitLinesLocked() {
        while let newline = buffer.firstIndex(of: UInt8(ascii: "\n")) {
            let line = Data(buffer[buffer.startIndex..<newline])
            buffer.removeSubrange(buffer.startIndex...newline)
            guard !line.isEmpty else { continue }
            routeLocked(line)
        }
    }

    private func routeLocked(_ line: Data) {
        guard let id = Self.responseId(in: line) else { return }
        // A reply with no matching entry is either a response to a request that
        // already timed out or the server's id-0 parse error; both are dropped
        // rather than delivered to an unrelated caller.
        guard let continuation = pending.removeValue(forKey: id) else { return }
        continuation.resume(returning: line)
    }

    private func scheduleTimeoutLocked(id: Int) {
        // Captures `self` on purpose: the timer must outlive this connection's
        // other references so the continuation is always resumed.
        queue.asyncAfter(deadline: .now() + timeout) {
            guard let continuation = self.pending.removeValue(forKey: id) else { return }
            continuation.resume(throwing: IPCError.timeout)
        }
    }

    private func failAllLocked(_ error: Error) {
        let waiting = pending
        pending.removeAll()
        for continuation in waiting.values {
            continuation.resume(throwing: error)
        }
    }

    private struct WireId: Decodable {
        let id: Int
    }

    private static func responseId(in data: Data) -> Int? {
        (try? JSONDecoder().decode(WireId.self, from: data))?.id
    }

    // MARK: - Write side

    private func writeLocked(_ frame: Data) throws {
        guard fd >= 0 else { throw IPCError.writeFailed }
        // One deadline covers the whole write, matching the read-side
        // contract: a peer that stops draining must surface `.timeout`
        // instead of parking this queue indefinitely (audit 0.7).
        let deadline = Date().addingTimeInterval(timeout)

        try frame.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { throw IPCError.writeFailed }
            var written = 0
            while written < raw.count {
                let remaining = deadline.timeIntervalSinceNow
                if remaining <= 0 { throw IPCError.timeout }

                let n = Darwin.write(fd, base.advanced(by: written), raw.count - written)
                if n > 0 {
                    written += n
                    continue
                }
                if errno == EINTR { continue }
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    var pfd = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
                    let waitMs = Int32(max(1, min(remaining * 1000, Double(Int32.max))))
                    let ready = poll(&pfd, 1, waitMs)
                    if ready < 0 && errno != EINTR { throw IPCError.writeFailed }
                    continue
                }
                // Peer gone. Drop the connection so the next call reconnects;
                // anything already waiting is failed by the reader's hang-up
                // path or by its own deadline.
                closeLocked()
                throw IPCError.writeFailed
            }
        }
    }
}
