import Darwin
import Foundation
import XCTest

@testable import ShareCLICore

/// PLAN.md:162-166 — "single long-lived connection with id→continuation map
/// and per-request cancellation. Validate by feeding a 1 MB line; second
/// connection attempt rejected."
///
/// The 1 MB line is a *server*-side contract and is covered end-to-end by the
/// Rust `ipc_line_limit` suite; what is asserted here is the client half: one
/// connection serves many calls, a second `connect()` is refused before it
/// reaches the socket, replies route by request id, and a request that runs out
/// of its own deadline is cancelled without disturbing anything else in flight.
final class IPCConnectionTests: XCTestCase {

    // MARK: - Acceptance: second connection attempt rejected

    func testSecondConnectionAttemptIsRejected() throws {
        let server = LineSidecarServer()
        defer { server.stop() }
        let connection = IPCConnection(socketPath: server.path, timeout: 2)

        XCTAssertNoThrow(try connection.connect(), "the first attempt must succeed")
        XCTAssertThrowsError(try connection.connect(), "a second attempt must be refused") {
            error in
            guard case IPCError.alreadyConnected = error else {
                return XCTFail("expected IPCError.alreadyConnected, got \(error)")
            }
        }

        XCTAssertTrue(
            server.waitUntilAccepted(1),
            "the first attempt must reach the listener"
        )
        XCTAssertEqual(
            server.acceptCount, 1,
            "the refused attempt must never reach the socket"
        )
    }

    // MARK: - Acceptance: one connection, many calls

    func testFiveCallsShareASingleLongLivedConnection() async throws {
        let server = LineSidecarServer()
        server.setResponder { id in echo(id) }
        defer { server.stop() }
        let connection = IPCConnection(socketPath: server.path, timeout: 2)

        for id in 1...5 {
            let reply: Data
            do {
                reply = try await connection.send(id: id, frame: requestFrame(id))
            } catch {
                XCTFail("request \(id) never got a reply: \(error); accepted=\(server.acceptCount)")
                return
            }
            let wire = try JSONDecoder().decode(IdOnly.self, from: reply)
            XCTAssertEqual(
                wire.id, id,
                "the reply for request \(id) must be the one routed back to it"
            )
        }

        XCTAssertEqual(
            server.acceptCount, 1,
            "five calls must ride one connection; per-call sockets would accept five times"
        )
    }

    // MARK: - Buffered read: a reply far larger than one byte still arrives whole

    func testLargeResponseIsDeliveredIntact() async throws {
        let payload = String(repeating: "a", count: 300 * 1024)
        let server = LineSidecarServer()
        server.setResponder { id in
            "{\"id\":\(id),\"result\":\"\(payload)\",\"error\":null}"
        }
        defer { server.stop() }

        let connection = IPCConnection(socketPath: server.path, timeout: 10)
        let reply = try await connection.send(id: 1, frame: requestFrame(1))
        let wire = try JSONDecoder().decode(StringResult.self, from: reply)

        XCTAssertEqual(wire.id, 1)
        XCTAssertEqual(
            wire.result.count, payload.count,
            "a 300 KiB reply must survive reassembly byte for byte"
        )
    }

    // MARK: - Per-request cancellation

    func testTimeoutCancelsOnlyThatRequestAndTheConnectionKeepsServing() async {
        let server = LineSidecarServer()
        server.setResponder { _ in nil } // accept the request, never answer it
        defer { server.stop() }

        let connection = IPCConnection(socketPath: server.path, timeout: 0.4)

        do {
            _ = try await connection.send(id: 1, frame: requestFrame(1))
            XCTFail("an unanswered request must not return a result")
        } catch let error as IPCError {
            guard case .timeout = error else {
                return XCTFail("expected IPCError.timeout, got \(error)")
            }
        } catch {
            XCTFail("expected IPCError.timeout, got \(error)")
        }

        // The deadline belongs to the request, not to the socket: the same
        // connection must still be usable, which also proves the timeout did
        // not silently force a reconnect.
        server.setResponder { id in echo(id) }
        do {
            let reply = try await connection.send(id: 2, frame: requestFrame(2))
            let wire = try JSONDecoder().decode(IdOnly.self, from: reply)
            XCTAssertEqual(wire.id, 2)
        } catch {
            XCTFail("the connection must keep serving after a peer-side timeout: \(error)")
        }

        XCTAssertEqual(server.acceptCount, 1, "a timeout must not tear the connection down")
    }

    func testConcurrentRequestsEachCancelOnTheirOwnDeadline() async {
        let server = LineSidecarServer()
        server.setResponder { _ in nil }
        defer { server.stop() }

        let connection = IPCConnection(socketPath: server.path, timeout: 0.4)

        let outcomes = await withTaskGroup(of: String.self) { group -> [String] in
            for id in 1...8 {
                group.addTask {
                    do {
                        _ = try await connection.send(id: id, frame: requestFrame(id))
                        return "unexpected-success"
                    } catch let error as IPCError {
                        if case .timeout = error { return "timeout" }
                        return "other:\(error)"
                    } catch {
                        return "error:\(error)"
                    }
                }
            }
            var collected: [String] = []
            for await outcome in group { collected.append(outcome) }
            return collected
        }

        XCTAssertEqual(outcomes.count, 8, "all eight requests must resolve")
        XCTAssertEqual(
            outcomes.filter { $0 == "timeout" }.count, 8,
            "every in-flight request must cancel on its own deadline, got \(outcomes)"
        )
    }

    // MARK: - Routing

    func testRepliesRouteByIdSoAnUnansweredRequestNeverInheritsAnother() async {
        let server = LineSidecarServer()
        // Answer id 2, ignore id 1 — an out-of-order/impossible-for-the-real-
        // server case that still has to be handled correctly by the map.
        server.setResponder { id in id == 2 ? echo(id) : nil }
        defer { server.stop() }

        let connection = IPCConnection(socketPath: server.path, timeout: 0.6)

        async let unanswered: Data = connection.send(id: 1, frame: requestFrame(1))
        async let answered: Data = connection.send(id: 2, frame: requestFrame(2))

        do {
            let reply = try await answered
            let wire = try JSONDecoder().decode(IdOnly.self, from: reply)
            XCTAssertEqual(wire.id, 2, "id 2's reply must land on id 2")
        } catch {
            XCTFail("id 2 was answered by the server and must resolve: \(error)")
        }

        do {
            _ = try await unanswered
            XCTFail("id 1 was never answered and must time out, not borrow id 2's reply")
        } catch let error as IPCError {
            guard case .timeout = error else {
                return XCTFail("expected IPCError.timeout for id 1, got \(error)")
            }
        } catch {
            XCTFail("expected IPCError.timeout for id 1, got \(error)")
        }
    }
}
