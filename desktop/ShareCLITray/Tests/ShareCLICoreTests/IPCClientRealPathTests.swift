import Foundation
import XCTest
@testable import ShareCLICore

/// Test-local flag shared between a detached `Task` and the test thread. A bare
/// captured `var` is a data race; this serializes access (audit warn-tier).
private final class LockedFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false

    func set() {
        lock.lock()
        value = true
        lock.unlock()
    }

    var isSet: Bool {
        lock.lock()
        defer { lock.unlock() }
        return value
    }
}

/// Real-path acceptance for the phase-0.5/0.6/0.7 BLOCKER fixes.
///
/// This is NOT the in-process synthetic server used by `IPCClientDeadlineTests`.
/// It drives the REAL `IPCClient` actor against a REAL external daemon: the real
/// `sharecli-ipc` sidecar binary, speaking NDJSON over a REAL Unix domain socket,
/// with REAL processes on the other end. Point `SHARECLI_TEST_IPC_SOCK` at an
/// isolated sidecar to run it; the suite skips when that is unset.
///
///     SHARECLI_TEST_IPC_SOCK=/tmp/iso.sock swift test --filter IPCClientRealPath
///
/// The wedge case needs no daemon at all: it uses a real listening socket that
/// accepts the connection and never answers, which is the exact production
/// failure the 0.7 bounded-read BLOCKER describes.
final class IPCClientRealPathTests: XCTestCase {

    /// Gated: only runs against a real, isolated sidecar.
    private func realClient() throws -> IPCClient {
        guard let path = ProcessInfo.processInfo.environment["SHARECLI_TEST_IPC_SOCK"] else {
            throw XCTSkip("SHARECLI_TEST_IPC_SOCK not set; start an isolated sharecli-ipc")
        }
        return IPCClient(socketPath: path, timeout: 5.0)
    }

    private func pidAlive(_ pid: UInt32) -> Bool {
        kill(pid_t(pid), 0) == 0
    }

    // MARK: - 0.7 real acceptance

    /// A real listener that accepts and never replies. The real client must
    /// surface `IPCError.timeout` inside its deadline, not hang, and a healthy
    /// client must still be served immediately afterwards: a wedged peer must
    /// not hold a shared probe slot forever.
    func test_wedged_daemon_times_out_and_does_not_starve_healthy_client() throws {
        let dir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("wedge-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let wedge = WedgeListener(path: dir.appendingPathComponent("wedged.sock").path)
        wedge.start()
        defer { wedge.stop() }

        let wedged = IPCClient(socketPath: wedge.path, timeout: 1.0)
        let t0 = Date()
        let wedgedExp = expectation(description: "wedged daemon must time out")
        // The flag is written from the detached `Task` and read on the test
        // thread; a bare `var` would be a data race that Swift 6 concurrency
        // checking flags (audit warn-tier). `LockedFlag` serializes it.
        let timedOut = LockedFlag()
        Task {
            defer { wedgedExp.fulfill() }
            do {
                _ = try await wedged.listProcesses()
                XCTFail("wedged daemon must never answer process.list")
            } catch IPCError.timeout {
                timedOut.set()
            } catch {
                XCTFail("expected IPCError.timeout, got \(error)")
            }
        }
        wait(for: [wedgedExp], timeout: 30)
        let elapsed = Date().timeIntervalSince(t0)
        XCTAssertTrue(timedOut.isSet, "wedged daemon must fail with IPCError.timeout")
        XCTAssertLessThan(
            elapsed, 5.0,
            "wedged daemon must fail near the 1.0s deadline, took \(elapsed)s"
        )

        // The healthy client uses a separate socket and a fresh connection, so
        // the wedge must not have consumed the shared bounded pool.
        if let path = ProcessInfo.processInfo.environment["SHARECLI_TEST_IPC_SOCK"] {
            let healthy = IPCClient(socketPath: path, timeout: 5.0)
            let healthyExp = expectation(description: "healthy client after wedge")
            Task {
                defer { healthyExp.fulfill() }
                do {
                    _ = try await healthy.listProcesses()
                } catch {
                    XCTFail("healthy client starved after wedge: \(error)")
                }
            }
            wait(for: [healthyExp], timeout: 30)
        }
    }

    // MARK: - 0.5 / 0.6 real acceptance

    /// Real spawn -> real live pid -> real kill, through the real client and the
    /// real sidecar. This is the 0.5 socket/permission/spawn surface and the 0.6
    /// stop path, end to end, with a process that really exists.
    func test_real_sidecar_spawn_then_kill_round_trip() throws {
        let client = try realClient()
        let exp = expectation(description: "real spawn + kill")
        Task {
            defer { exp.fulfill() }
            var pid: UInt32 = 0
            do {
                let result = try await client.spawn(payload: ProcessSpawnPayload(
                    name: "acceptance-sleep",
                    command: "/bin/sleep",
                    args: ["120"],
                    project: nil,
                    harness: nil,
                    cwd: nil
                ))
                XCTAssertTrue(result.success, "real spawn must succeed: \(result)")
                pid = result.pid
                XCTAssertGreaterThan(pid, 0, "real spawn must return a pid")
                XCTAssertTrue(pidAlive(pid), "spawned pid \(pid) must really exist")

                let listed = try await client.listProcesses()
                XCTAssertTrue(
                    listed.contains { $0.pid == pid },
                    "spawned pid \(pid) must appear in the real process list"
                )

                try await client.kill(pid: pid)
                // Real termination, not just a receipt.
                let deadline = Date().addingTimeInterval(10)
                while pidAlive(pid) && Date() < deadline {
                    Thread.sleep(forTimeInterval: 0.1)
                }
                XCTAssertFalse(pidAlive(pid), "killed pid \(pid) must really be gone")
            } catch {
                XCTFail("real spawn/kill round trip failed: \(error)")
                if pid > 0 { kill(pid_t(pid), SIGKILL) }
            }
        }
        wait(for: [exp], timeout: 90)
    }

    /// The 0.6 stop BLOCKER on the real daemon: killing an unknown pid must be
    /// reported as a failure, not silently accepted as success.
    func test_real_sidecar_rejects_kill_of_unknown_pid() throws {
        let client = try realClient()
        let exp = expectation(description: "unknown pid rejected")
        Task {
            defer { exp.fulfill() }
            do {
                try await client.kill(pid: 999_999)
                XCTFail("killing an unknown pid must be reported as an error")
            } catch {
                // Expected: the real sidecar refuses an unmanaged pid.
            }
        }
        wait(for: [exp], timeout: 30)
    }

    /// The real 0.7 decode surface: the live payload must satisfy the exact
    /// decoder contract the tray uses.
    func test_real_sidecar_effectiveness_matches_decoder() throws {
        let client = try realClient()
        let exp = expectation(description: "effectiveness decodes")
        Task {
            defer { exp.fulfill() }
            do {
                let eff = try await client.poolEffectiveness()
                XCTAssertGreaterThan(eff.sampled_at, 0, "live payload must carry a sample time")
                // Meters are unsigned counters; recomputing the page's hit rate
                // must stay inside 0...100.
                let total = eff.coalesce.hits + eff.coalesce.misses
                let rate = total > 0 ? Double(eff.coalesce.hits) / Double(total) * 100.0 : 0.0
                XCTAssertTrue((0.0...100.0).contains(rate), "hit rate \(rate) must be in range")
            } catch {
                XCTFail("real effectiveness decode failed: \(error)")
            }
        }
        wait(for: [exp], timeout: 30)
    }
}

/// A real listening Unix socket that accepts connections and never writes a
/// reply -- the production wedge the 0.7 bounded-read fix must survive. Built on
/// the shared `runAcceptLoop`/`closeListeningSocket` helpers so its teardown
/// joins the accept thread before closing the listener, instead of leaking a
/// 600 s sleeper thread and the accepted connection (audit warn-tier).
final class WedgeListener {
    let path: String
    private var listenFD: Int32 = -1
    private let lock = NSLock()
    private var accepted: [Int32] = []
    private var stopped = false
    private var acceptLoop: AcceptLoop?

    init(path: String) { self.path = path }

    func start() {
        listenFD = makeListeningUnixSocket(
            at: path,
            backlog: 8,
            listenFailureMessage: "listen() failed"
        )
        let server = self
        acceptLoop = runAcceptLoop(
            listenFD: listenFD,
            lock: lock,
            isStopped: { server.stopped },
            onAccept: { client in
                // Hold the connection open and deliberately send nothing so the
                // client blocks in read() until its own deadline fires. The fd
                // is closed by `stop()` after the accept thread has been joined.
                server.lock.lock()
                server.accepted.append(client)
                server.lock.unlock()
            },
            serve: { _ in }
        )
    }

    func stop() {
        lock.lock()
        stopped = true
        let conns = accepted
        accepted = []
        lock.unlock()
        // Join the accept thread before closing the fds it may still be using.
        if let acceptLoop { joinAcceptLoop(acceptLoop) }
        closeListeningSocket(listenFD, connections: conns, path: path)
    }
}
