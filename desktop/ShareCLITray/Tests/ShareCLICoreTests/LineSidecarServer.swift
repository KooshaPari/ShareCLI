import Darwin
import Foundation

/// Shared NDJSON test listener for the IPC connection suites. One connection
/// per peer, many requests per connection, and `acceptCount` records how many
/// times the listener had to accept — the observable difference between a
/// client that opens a socket per call and one that keeps a connection open.

// MARK: - Helpers

func requestFrame(_ id: Int) -> Data {
    Data("{\"id\":\(id),\"method\":\"health.status\",\"params\":{}}\n".utf8)
}

func echo(_ id: Int) -> String {
    "{\"id\":\(id),\"result\":{},\"error\":null}"
}

struct IdOnly: Decodable {
    let id: Int
}

struct StringResult: Decodable {
    let id: Int
    let result: String
}

/// A Unix-socket listener that keeps answering on one connection for as long as
/// the peer keeps it open. `acceptCount` is what proves connection reuse: the
/// old per-call client accepted once per request.
final class LineSidecarServer: @unchecked Sendable {
    typealias Responder = (Int) -> String?

    let path: String

    private let listenFD: Int32
    private let lock = NSLock()
    private var connections: [Int32] = []
    private var stopped = false
    private var responder: Responder = { id in echo(id) }

    init() {
        path = "/tmp/sharecli-line-sidecar-\(UUID().uuidString).sock"
        listenFD = makeListeningUnixSocket(
            at: path,
            backlog: 16,
            listenFailureMessage: "listen() failed"
        )

        let server = self
        runAcceptLoop(
            listenFD: listenFD,
            lock: lock,
            isStopped: { server.stopped },
            onAccept: { fd in
                server.lock.lock()
                server.connections.append(fd)
                server.lock.unlock()
                // The listener is non-blocking so the accept loop can poll for
                // `stopped`. Normalise the accepted socket back to blocking —
                // `serve` wants to sit in `read` until the peer speaks or goes
                // away — and tolerate `EAGAIN` below regardless, because an
                // unhandled one ends the loop after the first request and every
                // later request on that connection goes unanswered.
                let fileFlags = fcntl(fd, F_GETFL, 0)
                _ = fcntl(fd, F_SETFL, fileFlags & ~O_NONBLOCK)
                // A separate thread per connection, so a client that does
                // reconnect cannot be blocked behind the one it left open.
                Thread { server.serve(fd) }.start()
            },
            serve: { _ in }
        )
    }

    /// Block (bounded) until at least `count` connections have been accepted.
    /// `connect()` returns from the kernel handshake before the listener's
    /// accept loop has dequeued it, so an immediate count reads zero.
    func waitUntilAccepted(_ count: Int, seconds: TimeInterval = 2) -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if acceptCount >= count { return true }
            usleep(10_000)
        }
        return acceptCount >= count
    }

    var acceptCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return connections.count
    }

    func setResponder(_ responder: @escaping Responder) {
        lock.lock()
        self.responder = responder
        lock.unlock()
    }

    private func currentResponder() -> Responder {
        lock.lock()
        defer { lock.unlock() }
        return responder
    }

    /// Read NDJSON frames for as long as the peer keeps the socket open.
    private func serve(_ fd: Int32) {
        var chunk = [UInt8](repeating: 0, count: 65_536)
        var pending: [UInt8] = []
        while true {
            let n = read(fd, &chunk, chunk.count)
            if n == 0 { return } // peer closed
            if n < 0 {
                if errno == EINTR { continue }
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    usleep(5_000)
                    continue
                }
                return
            }
            pending.append(contentsOf: chunk[0..<n])
            if pending.count > 65_536 { return }

            while let newline = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let line = Array(pending[0..<newline])
                pending.removeSubrange(0...newline)
                handle(line, on: fd)
            }
        }
    }

    private func handle(_ line: [UInt8], on fd: Int32) {
        guard
            let object = try? JSONSerialization.jsonObject(with: Data(line)),
            let dict = object as? [String: Any],
            let id = dict["id"] as? Int
        else { return }

        guard let body = currentResponder()(id) else { return }
        writeAll(Data("\(body)\n".utf8), to: fd)
    }

    /// Replies can be far larger than the socket buffer, so a single `write`
    /// would be short and silently truncate the frame.
    private func writeAll(_ bytes: Data, to fd: Int32) {
        bytes.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            var written = 0
            while written < raw.count {
                let n = Darwin.write(fd, base.advanced(by: written), raw.count - written)
                if n > 0 {
                    written += n
                    continue
                }
                if errno == EINTR { continue }
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    var pfd = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
                    _ = poll(&pfd, 1, 50)
                    continue
                }
                return
            }
        }
    }

    func stop() {
        lock.lock()
        stopped = true
        let open = connections
        connections = []
        lock.unlock()
        closeListeningSocket(listenFD, connections: open, path: path)
    }
}
