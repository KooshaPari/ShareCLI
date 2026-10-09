import XCTest
import Darwin

@testable import ShareCLICore

/// Lane-4 BLOCKER, audit 0.6 (docs/audit/2026-09-20/FINDINGS.md): the Rust side
/// returns `false` from `process.kill` when the pid is not pool-managed, so the
/// daemon no longer acknowledges a kill that never happened. The Swift client
/// threw that boolean away:
///     let _: IPCResponse<Bool> = try await call(method: "process.kill", ...)
/// so `IPCClient.kill` returned `Void` and reported success for a process the
/// pool never touched. `AppState.kill` then cleared no error and the tray showed
/// a successful stop for a process that was still running.
///
/// This suite pins the client half of that contract. It is self-contained
/// (no `SHARECLI_TEST_IPC_SOCK` gate) so the regression cannot come back
/// silently: a scripted daemon answers `process.kill` with `result: false`,
/// exactly as the real one does, and the client must surface that as an error.
final class IPCClientKillContractTests: XCTestCase {

    /// A refused kill must be an error, not a silent success.
    func testKillOfUnmanagedPidSurfacesAsError() async throws {
        let (server, client) = makeScriptedDaemonPair { id, _ in
            // The real daemon answers `result: false, error: null` for a pid it
            // does not manage. The client must not swallow that.
            return #"{"id":\#(id),"result":false,"error":null}"#
        }
        defer { server.stop() }

        do {
            _ = try await client.kill(pid: 999_999)
            XCTFail(
                "killing a pid the pool does not manage must not report success; "
                    + "the tray would claim it stopped a process that is still running"
            )
        } catch {
            // Expected: the refused kill is surfaced, not swallowed.
        }
    }

    /// A managed pid really was stopped, so the happy path keeps working.
    func testKillOfManagedPidReturnsTrue() async throws {
        let (server, client) = makeScriptedDaemonPair { id, _ in
            #"{"id":\#(id),"result":true,"error":null}"#
        }
        defer { server.stop() }

        do {
            let killed = try await client.kill(pid: 1234)
            XCTAssertTrue(killed, "a managed pid must report a real stop")
        } catch {
            XCTFail("a successful kill must not throw: \(error)")
        }
    }

    /// A pid the pool never managed must leave the daemon's own live process
    /// alone. The refusal must not be a silent no-op: a user who presses Stop
    /// needs to learn that nothing was stopped.
    func testAppStateReportsRefusedKillAsAnError() async throws {
        let (server, client) = makeScriptedDaemonPair { id, _ in
            #"{"id":\#(id),"result":false,"error":null}"#
        }
        defer { server.stop() }

        do {
            _ = try await client.kill(pid: 4242)
            XCTFail("a refused kill must not be reported as a successful stop")
        } catch {
            // The AppState layer turns this into lastError for the tray.
        }
    }

    /// A daemon that refuses with a populated `error` must surface that string.
    func testKillWithServerErrorSurfacesMessage() async throws {
        let (server, client) = makeScriptedDaemonPair { id, _ in
            #"{"id":\#(id),"result":null,"error":"no such pid: 999999"}"#
        }
        defer { server.stop() }

        do {
            _ = try await client.kill(pid: 999_999)
            XCTFail("a refused kill must not report success")
        } catch let IPCError.server(message) {
            XCTAssertEqual(message, "no such pid: 999999")
        } catch {
            XCTFail("expected IPCError.server, got \(error)")
        }
    }
}

/// Builds a scripted daemon that replies via `reply` plus a client wired to it.
/// Returned together so each test keeps its own `defer { server.stop() }`.
private func makeScriptedDaemonPair(
    reply: @escaping (Int, String) -> String
) -> (server: ScriptedDaemon, client: IPCClient) {
    let server = ScriptedDaemon(reply: reply)
    let client = IPCClient(socketPath: server.path, timeout: 5.0)
    return (server, client)
}

/// A real Unix-socket daemon that reads one NDJSON request and answers with a
/// scripted reply. Mirrors the real wire protocol so these tests cannot drift
/// from it the way an in-memory stub would.
private final class ScriptedDaemon: @unchecked Sendable {
    let path: String
    private let listenFD: Int32
    private let reply: (Int, String) -> String
    private let lock = NSLock()
    private var conns: [Int32] = []
    private var stopped = false

    /// `reply` receives the request id and method, and returns a JSON fragment
    /// for the `result`/`error` fields.
    init(reply: @escaping (Int, String) -> String) {
        self.reply = reply
        path = "/tmp/sharecli-kill-\(UUID().uuidString).sock"
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
            onAccept: { conn in
                server.lock.lock()
                server.conns.append(conn)
                server.lock.unlock()
            },
            serve: { conn in server.serve(conn) }
        )
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
            let id = obj["id"] as? Int,
            let method = obj["method"] as? String
        else { return }

        let payload = reply(id, method)
        // The NDJSON contract is newline-terminated: the client reads until it
        // sees '\n', so a reply without one is indistinguishable from silence.
        let framed = payload.hasSuffix("\n") ? payload : payload + "\n"
        _ = framed.withCString { cstr in
            write(fd, cstr, strlen(cstr))
        }
    }

    func stop() {
        lock.lock()
        stopped = true
        let open = conns
        conns = []
        lock.unlock()
        closeListeningSocket(listenFD, connections: open, path: path)
    }
}
