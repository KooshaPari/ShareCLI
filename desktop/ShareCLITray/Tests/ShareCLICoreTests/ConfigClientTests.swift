import XCTest

@testable import ShareCLICore

/// Headless tests for the IPC config boundary used by the tray's Runtime tab.
///
/// The config round-trip tests talk to a real `sharecli-ipc` process rather
/// than a stub, so they exercise the actual NDJSON wire protocol and the Rust
/// validation gate. Point `SHARECLI_TEST_IPC_SOCK` at an isolated daemon to
/// run them; without it the suite skips so `swift test` stays green in CI.
///
///     SHARECLI_TEST_IPC_SOCK=/tmp/itest.sock swift test
///
/// What these cover: `getConfig()` is the exact source the Runtime rows prefill
/// from, so if it stops exposing `runtime.max_memory_mb` the original
/// "must be greater than 0" regression returns. `setConfig` must accept a valid
/// cap and must reject a zero cap rather than persisting it.
final class ConfigClientTests: XCTestCase {
    private var socketPath: String!

    override func setUpWithError() throws {
        guard let path = ProcessInfo.processInfo.environment["SHARECLI_TEST_IPC_SOCK"] else {
            throw XCTSkip(
                "SHARECLI_TEST_IPC_SOCK not set; start an isolated sharecli-ipc to exercise the config boundary"
            )
        }
        socketPath = path
    }

    private func makeClient() -> IPCClient {
        IPCClient(socketPath: socketPath)
    }

    /// Read the runtime caps out of the live config, mirroring what the
    /// Runtime subpage does when it prefills its editors.
    private func runtimeCaps() async throws -> (memory: Int?, processes: Int?) {
        let data = try await makeClient().getConfig()
        let root = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let runtime = root?["runtime"] as? [String: Any]
        return (runtime?["max_memory_mb"] as? Int, runtime?["max_processes"] as? Int)
    }

    func testGetConfigExposesRuntimeCapsUsedByPrefill() async throws {
        let caps = try await runtimeCaps()
        XCTAssertNotNil(caps.memory, "config.get must expose runtime.max_memory_mb for the prefill")
        XCTAssertNotNil(caps.processes, "config.get must expose runtime.max_processes for the prefill")
        XCTAssertGreaterThan(caps.memory ?? 0, 0, "a prefilled cap must be usable, not zero")
        XCTAssertGreaterThan(caps.processes ?? 0, 0, "a prefilled cap must be usable, not zero")
    }

    func testSetConfigAcceptsValidCapAndPersists() async throws {
        try await makeClient().setConfig(key: "runtime.max_memory_mb", value: .int(8192))
        let caps = try await runtimeCaps()
        XCTAssertEqual(caps.memory, 8192, "an accepted patch must be readable back")
    }

    func testSetConfigRejectsZeroCapAndLeavesValueIntact() async throws {
        try await makeClient().setConfig(key: "runtime.max_memory_mb", value: .int(4096))

        do {
            try await makeClient().setConfig(key: "runtime.max_memory_mb", value: .int(0))
            XCTFail("config.set accepted runtime.max_memory_mb = 0; the tray warning promises rejection")
        } catch {
            // Expected: the Rust validation gate refuses the patch.
        }

        let caps = try await runtimeCaps()
        XCTAssertEqual(caps.memory, 4096, "a rejected patch must not change the stored value")
    }

    func testSetConfigRejectsZeroProcessCap() async throws {
        do {
            try await makeClient().setConfig(key: "runtime.max_processes", value: .int(0))
            XCTFail("config.set accepted runtime.max_processes = 0")
        } catch {
            // Expected.
        }
    }
}
