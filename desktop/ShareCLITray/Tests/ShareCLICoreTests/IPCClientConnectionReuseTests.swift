import Foundation
import XCTest

@testable import ShareCLICore

/// PLAN.md:166 — the client half of "single long-lived connection": many calls
/// must ride one socket instead of opening one per call.
///
/// This deliberately talks to `IPCClient`, the type the audit item is written
/// against, rather than to `IPCConnection`. That keeps it runnable against the
/// pre-refactor implementation, where it fails with one accept per call — the
/// red half of the receipt for task 1.6b.
final class IPCClientConnectionReuseTests: XCTestCase {
    func testThreeCallsRideOneConnection() async {
        let server = LineSidecarServer()
        server.setResponder { id in "{\"id\":\(id),\"result\":[],\"error\":null}" }
        defer { server.stop() }

        let client = IPCClient(socketPath: server.path, timeout: 2)
        for attempt in 1...3 {
            do {
                _ = try await client.listProcesses()
            } catch {
                XCTFail("call \(attempt) failed: \(error); accepted=\(server.acceptCount)")
                return
            }
        }

        // `connect()` completes in the kernel before the accept loop dequeues
        // it, so give the listener a moment before counting.
        _ = server.waitUntilAccepted(1)
        XCTAssertEqual(
            server.acceptCount, 1,
            "three calls must reuse one connection; the per-call client accepts once per call"
        )
    }
}
