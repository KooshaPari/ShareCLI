import XCTest
import Darwin

@testable import ShareCLICore

/// Lane-2 BLOCKER, audit 0.7 (docs/audit/2026-09-20/FINDINGS.md:33): the IPC
/// read loop was `while true { Darwin.read(fd, &byte, 1) }` with no deadline,
/// so a sidecar that accepts and never answers pinned a
/// `DispatchQueue.global(qos: .utility)` blocking closure forever. Measured
/// ceiling of that pool = 64: a wedged sidecar saturates it in ~5 minutes and
/// after that the supervisor's recovery probes never run again.
///
/// These tests reproduce the wedge in-process: `WedgedSidecarServer` accepts
/// connections and never writes a byte (the exact failure mode), and the
/// probe must fail with `IPCError.timeout` instead of hanging. The flood case
/// exceeds the measured 64 ceiling to prove the pool still serves new work
/// afterwards — that is the "supervisor recovers" half of the receipt.
///
/// No `SHARECLI_TEST_IPC_SOCK` gate: these servers are self-contained
/// listeners and must run in every `swift test`.
final class IPCClientDeadlineTests: XCTestCase {

    // MARK: - Receipt 1: block-forever probe fails with IPCError.timeout

    func testBlockForeverProbeFailsWithTimeoutNotAHang() {
        let server = WedgedSidecarServer()
        defer { server.stop() }
        let client = IPCClient(socketPath: server.path, timeout: 0.5)
        let gate = TestGate()
        let probeReturned = expectation(description: "wedged probe resolves")

        Task {
            do {
                _ = try await client.health()
                if !gate.isFinished {
                    XCTFail("a sidecar that never answers must not produce a health() result")
                }
            } catch let error as IPCError {
                if gate.isFinished { return }
                if case .timeout = error {
                    probeReturned.fulfill()
                } else {
                    XCTFail("expected IPCError.timeout, got \(error)")
                }
            } catch {
                if !gate.isFinished {
                    XCTFail("expected IPCError.timeout, got \(error)")
                }
            }
        }

        let started = Date()
        let outcome = XCTWaiter.wait(for: [probeReturned], timeout: 5)
        gate.markFinished()
        XCTAssertEqual(
            outcome, .completed,
            "block-forever probe must fail with IPCError.timeout within the deadline; "
                + "a hang means the read loop still has no deadline"
        )
        XCTAssertLessThan(
            Date().timeIntervalSince(started), 4.5,
            "the timeout must fire close to the configured deadline (0.5s), not near the waiter bound"
        )
    }

    // MARK: - Receipt 2: >64 wedged probes resolve, then new work still runs

    /// 70 concurrent wedged probes > the measured 64 ceiling. If each one
    /// blocked a global-utility closure forever, the last 6+ would never even
    /// start, the pool would stay saturated, and the supervisor's next probe
    /// (a fresh call on the same pool) would never run. Both halves are
    /// asserted: every wedged probe must end in `.timeout`, and a healthy
    /// call must still be served afterwards.
    func testWedgedProbeFloodResolvesAndThePoolServesFollowUpWork() {
        let wedged = WedgedSidecarServer()
        defer { wedged.stop() }
        let healthy = HealthyConfigServer()
        defer { healthy.stop() }

        let wedgedClient = IPCClient(socketPath: wedged.path, timeout: 0.5)
        let healthyClient = IPCClient(socketPath: healthy.path, timeout: 2.0)
        let gate = TestGate()

        let probesResolved = expectation(description: "70 wedged probes all end in .timeout")
        probesResolved.expectedFulfillmentCount = 70

        for i in 0..<70 {
            Task {
                do {
                    _ = try await wedgedClient.health()
                    if !gate.isFinished {
                        XCTFail("wedged probe \(i) returned success; the sidecar never answered")
                    }
                } catch let error as IPCError {
                    if gate.isFinished { return }
                    if case .timeout = error {
                        probesResolved.fulfill()
                    } else {
                        XCTFail("wedged probe \(i): expected .timeout, got \(error)")
                    }
                } catch {
                    if !gate.isFinished {
                        XCTFail("wedged probe \(i): expected IPCError.timeout, got \(error)")
                    }
                }
            }
        }

        let floodOutcome = XCTWaiter.wait(for: [probesResolved], timeout: 10)
        XCTAssertEqual(
            floodOutcome, .completed,
            "all 70 wedged probes must resolve .timeout; unfulfilled means the dispatch pool is wedged"
        )

        // Recovery half: a brand-new call on the shared pool must still be
        // served. This is the supervisor's next tick after the wedge.
        let followUp = expectation(description: "pool serves a healthy call after the flood")
        Task {
            do {
                _ = try await healthyClient.getConfig()
                followUp.fulfill()
            } catch {
                if !gate.isFinished {
                    XCTFail("follow-up call on a healthy server failed: \(error)")
                }
            }
        }
        let followOutcome = XCTWaiter.wait(for: [followUp], timeout: 5)
        gate.markFinished()
        XCTAssertEqual(
            followOutcome, .completed,
            "the dispatch pool must serve new work after >64 wedged probes resolved"
        )
    }
}

// MARK: - Test bookkeeping

/// Suppresses failure records from tasks that only resolve after the test's
/// waiter already returned (happens exactly once, in the pre-fix red run,
/// when teardown closes the wedged sockets and releases the blocked reads).
private final class TestGate: @unchecked Sendable {
    private let lock = NSLock()
    private var finished = false

    func markFinished() {
        lock.lock()
        finished = true
        lock.unlock()
    }

    var isFinished: Bool {
        lock.lock()
        defer { lock.unlock() }
        return finished
    }
}

// MARK: - In-process wedged sidecar

/// A Unix-socket listener that accepts connections and never writes a byte —
/// the exact sidecar wedge the audit measured (accepts, then no answer).
private final class WedgedSidecarServer: @unchecked Sendable {
    let path: String
    private let listenFD: Int32
    private let lock = NSLock()
    private var accepted: [Int32] = []
    private var stopped = false

    init() {
        path = "/tmp/sharecli-wedged-\(UUID().uuidString).sock"
        unlink(path)

        listenFD = socket(AF_UNIX, SOCK_STREAM, 0)
        precondition(listenFD >= 0, "socket() failed")

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        let copied = path.withCString { cstr -> Bool in
            withUnsafeMutableBytes(of: &addr.sun_path) { raw in
                guard let base = raw.baseAddress else { return false }
                let limit = min(capacity, raw.count)
                memset(base, 0, limit)
                guard strlen(cstr) < limit else { return false }
                strncpy(base.assumingMemoryBound(to: CChar.self), cstr, limit - 1)
                return true
            }
        }
        precondition(copied, "socket path too long")
        precondition(
            bind(listenFD, withUnsafePointer(to: &addr) { ptr in
                ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { $0 }
            }, socklen_t(MemoryLayout<sockaddr_un>.size)) == 0,
            "bind() failed"
        )
        precondition(listen(listenFD, 128) == 0, "listen() failed — 128 must cover the 70-probe flood")

        let flags = fcntl(listenFD, F_GETFL, 0)
        _ = fcntl(listenFD, F_SETFL, flags | O_NONBLOCK)

        let server = self
        let thread = Thread {
            while true {
                server.lock.lock()
                if server.stopped {
                    server.lock.unlock()
                    return
                }
                server.lock.unlock()

                let conn = accept(server.listenFD, nil, nil)
                if conn >= 0 {
                    server.lock.lock()
                    server.accepted.append(conn)
                    server.lock.unlock()
                } else if errno == EAGAIN || errno == EWOULDBLOCK {
                    usleep(20_000)
                } else {
                    return
                }
            }
        }
        thread.start()
    }

    func stop() {
        lock.lock()
        stopped = true
        let conns = accepted
        accepted = []
        lock.unlock()
        for fd in conns {
            Darwin.close(fd)
        }
        Darwin.close(listenFD)
        unlink(path)
    }
}

// MARK: - In-process healthy sidecar (config.get stub)

/// Accepts, reads one NDJSON request, answers `config.get` with an empty
/// object so the new poll-based read loop is also exercised on the happy path.
private final class HealthyConfigServer: @unchecked Sendable {
    let path: String
    private let listenFD: Int32
    private let lock = NSLock()
    private var conns: [Int32] = []
    private var stopped = false

    init() {
        path = "/tmp/sharecli-healthy-\(UUID().uuidString).sock"
        unlink(path)

        listenFD = socket(AF_UNIX, SOCK_STREAM, 0)
        precondition(listenFD >= 0, "socket() failed")

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        let copied = path.withCString { cstr -> Bool in
            withUnsafeMutableBytes(of: &addr.sun_path) { raw in
                guard let base = raw.baseAddress else { return false }
                let limit = min(capacity, raw.count)
                memset(base, 0, limit)
                guard strlen(cstr) < limit else { return false }
                strncpy(base.assumingMemoryBound(to: CChar.self), cstr, limit - 1)
                return true
            }
        }
        precondition(copied, "socket path too long")
        precondition(
            bind(listenFD, withUnsafePointer(to: &addr) { ptr in
                ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { $0 }
            }, socklen_t(MemoryLayout<sockaddr_un>.size)) == 0,
            "bind() failed"
        )
        precondition(listen(listenFD, 16) == 0, "listen() failed")

        let flags = fcntl(listenFD, F_GETFL, 0)
        _ = fcntl(listenFD, F_SETFL, flags | O_NONBLOCK)

        let server = self
        let thread = Thread {
            while true {
                server.lock.lock()
                if server.stopped {
                    server.lock.unlock()
                    return
                }
                server.lock.unlock()

                let conn = accept(server.listenFD, nil, nil)
                guard conn >= 0 else {
                    if errno == EAGAIN || errno == EWOULDBLOCK {
                        usleep(20_000)
                        continue
                    }
                    return
                }
                server.lock.lock()
                server.conns.append(conn)
                server.lock.unlock()
                server.serve(conn)
            }
        }
        thread.start()
    }

    private func serve(_ fd: Int32) {
        var buffer = [UInt8](repeating: 0, count: 65_536)
        var request = Data()
        while true {
            let n = read(fd, &buffer, buffer.count)
            if n <= 0 { return }
            request.append(contentsOf: buffer[0..<n])
            if request.contains(UInt8(ascii: "\n")) { break }
            if request.count > 65_536 { return }
        }
        guard
            let lineEnd = request.firstIndex(of: UInt8(ascii: "\n")),
            let json = try? JSONSerialization.jsonObject(with: request[0..<lineEnd]),
            let obj = json as? [String: Any],
            let id = obj["id"] as? Int
        else { return }

        let reply = "{\"id\":\(id),\"result\":{},\"error\":null}\n"
        _ = reply.withCString { cstr in
            write(fd, cstr, strlen(cstr))
        }
    }

    func stop() {
        lock.lock()
        stopped = true
        let open = conns
        conns = []
        lock.unlock()
        for fd in open {
            Darwin.close(fd)
        }
        Darwin.close(listenFD)
        unlink(path)
    }
}
