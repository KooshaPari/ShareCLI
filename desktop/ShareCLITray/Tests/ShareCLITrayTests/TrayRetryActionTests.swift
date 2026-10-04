import XCTest

@testable import ShareCLITray

/// PLAN.md:228-231 — task 1.22 (lane 9, P1):
/// "Tray: Retry buttons (lane 9, P1)
///  - One `Retry` button per failed action; replace developer strings with
///    actionable copy; localized key for 'Not connected to sharecli-ipc' →
///    'Sidecar not running — Start'."
///
/// FR: UNKNOWN — PLAN.md:228-231 names no FR/AC for task 1.22. Nearest honest
/// anchor is FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which
/// governs the tray surface as a whole but not per-action recovery copy.
/// Recorded as UNKNOWN rather than inventing an id.
///
/// Scope: the *copy + recovery contract* consumed by the 5 tray pages that
/// surface a failed IPC action (HealthPage, PoolPage, PoolEffectivenessPage,
/// ConfigPage, LogsPage). The SwiftUI rendering of each banner is
/// compile-verified by the ShareCLITray target; the copy model and the
/// "no developer string remains" invariant are asserted below.
///
/// Site inventory (8 failed-action sites across the 5 owned files):
///   1–3. HealthPage Memory/Thermal/Host-watch disconnect banners
///     4. PoolPage capacity-footnote disconnect banner
///     5. PoolEffectivenessPage disconnect banner
///     6. ConfigPage live-preview disconnected placeholder
///   7–8. LogsPage open-log-file + export failures
final class TrayRetryActionTests: XCTestCase {

    // MARK: - AC: actionable disconnected copy replaces the developer string

    func testDisconnectedCopyIsActionable() {
        XCTAssertEqual(RetryCopy.disconnectedTitle, "Sidecar not running")
        XCTAssertEqual(RetryCopy.disconnectedStartLabel, "Start")
        XCTAssertEqual(RetryCopy.disconnectedInline, "Sidecar not running — Start")
    }

    func testRenderedCopyIsFreeOfDeveloperStrings() {
        // The retired literal must not appear in any string a page can render.
        for copy in RetryCopy.allCopy {
            XCTAssertFalse(
                copy.contains(RetryCopy.retiredDeveloperString),
                "rendered copy still contains the retired developer string: \(copy)"
            )
            XCTAssertFalse(
                copy.lowercased().contains("sharecli-ipc"),
                "rendered copy still leaks the internal socket name: \(copy)"
            )
        }
    }

    func testRetiredDeveloperStringExact() {
        // Guard the invariant the source scan below depends on.
        XCTAssertEqual(RetryCopy.retiredDeveloperString, "Not connected to sharecli-ipc")
    }

    // MARK: - AC: one recovery affordance per failure kind

    func testLogsFailureKindsEachCarryOneRetryAffordance() {
        let open = LogsFailure.openLogFile("/tmp/x.log: permission denied")
        XCTAssertEqual(open.retryLabel, "Retry")
        XCTAssertTrue(open.actionHint.lowercased().contains("open"))
        XCTAssertTrue(open.message.contains("permission denied"))

        let export = LogsFailure.export("%: disk full")
        XCTAssertEqual(export.retryLabel, "Retry")
        XCTAssertTrue(export.actionHint.lowercased().contains("export"))
        XCTAssertTrue(export.message.contains("disk full"))
    }

    func testEveryFailureKindHasExactlyOneActionLabel() {
        // "One Retry button per failed action" — a single label per kind.
        let kinds: [LogsFailure] = [.openLogFile("e"), .export("e")]
        for kind in kinds {
            XCTAssertEqual(kind.actionTitles.count, 1, "expected exactly one action for \(kind)")
            XCTAssertEqual(kind.actionTitles.first, kind.retryLabel)
        }
    }

    // MARK: - AC: source invariant — no page still renders the developer string

    func testOwnedSourceFilesNoLongerRenderDeveloperString() throws {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // ShareCLITrayTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // ShareCLITray
            .appendingPathComponent("Sources/ShareCLITray")

        let owned = [
            "HealthPage.swift",
            "PoolPage.swift",
            "PoolEffectivenessPage.swift",
            "ConfigPage.swift",
            "LogsPage.swift",
        ]

        for name in owned {
            let url = sourcesDir.appendingPathComponent(name)
            let text = try String(contentsOf: url, encoding: .utf8)
            XCTAssertFalse(
                text.contains("\"\(RetryCopy.retiredDeveloperString)\""),
                "\(name) still renders the retired developer string"
            )
        }
    }

    func testOwnedSourceFilesWireARecoveryAffordance() throws {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources/ShareCLITray")

        // Each page must reach the shared copy model rather than a local string.
        let owned = [
            "HealthPage.swift",
            "PoolPage.swift",
            "PoolEffectivenessPage.swift",
            "ConfigPage.swift",
            "LogsPage.swift",
        ]
        for name in owned {
            let text = try String(
                contentsOf: sourcesDir.appendingPathComponent(name),
                encoding: .utf8
            )
            XCTAssertTrue(
                text.contains("RetryCopy.") || text.contains("LogsFailure"),
                "\(name) does not use the shared retry copy / failure model"
            )
        }
    }
}
