import XCTest

@testable import ShareCLITray

/// PLAN.md:242-243 — task 1.26 (lane 9, P1):
/// "HelpSheet shortcuts complete — Document ⌘1..⌘8, ⌘K, ⌘F, ⌘R, ⌘W, ⌘/,
///  ⌘, with accurate platform markers."
///
/// FR: UNKNOWN — PLAN.md names no FR/AC for task 1.26. Nearest honest anchor is
/// FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which governs the
/// tray surface as a whole but not keyboard documentation. Recorded as UNKNOWN
/// rather than inventing an id.
///
/// Scope: HelpSheet's *shortcut data model* — every shortcut that is actually
/// wired in DashboardView.attachShortcutMonitor must appear in HelpSheet.shortcuts
/// with the correct label. The SwiftUI rendering is compile-verified by the
/// ShareCLITray target; the data contract is asserted below.
///
/// Platform: macOS — all keyboard markers use ⌘ (Command) per macOS convention.
final class HelpSheetShortcutTests: XCTestCase {

    // MARK: - Helpers

    /// Build a key→label lookup from the actual HelpSheet data.
    private var shortcutMap: [String: String] {
        Dictionary(
            uniqueKeysWithValues: HelpSheet.shortcuts.map { ($0.key, $0.label) }
        )
    }

    // MARK: - AC: ⌘1..⌘8 page shortcuts match Section.allCases order

    func testCmd1ToCmd8AreAllDocumented() {
        // The shortcut monitor in DashboardView.attachShortcutMonitor handles
        // Cmd+1 through Cmd+Section.allCases.count (8 pages). HelpSheet must
        // document all 8.
        let map = shortcutMap
        let pages = DashboardView.Section.allCases
        for (i, section) in pages.enumerated() {
            let key = "⌘\(i + 1)"
            XCTAssertNotNil(
                map[key],
                "HelpSheet must document \(key) for \(section.rawValue) page"
            )
        }
    }

    func testPageShortcutCount() {
        let pageKeys = HelpSheet.shortcuts
            .map(\.key)
            .filter { key in
                // Match exactly ⌘1 through ⌘8 (digit after ⌘).
                key.hasPrefix("⌘") && key.count == 2
                    && key.last.map(\.isNumber) == true
            }
        XCTAssertEqual(
            pageKeys.count, 8,
            "HelpSheet must document exactly 8 page navigation shortcuts (⌘1..⌘8), found \(pageKeys.count)"
        )
    }

    // MARK: - AC: page labels match Section.allCases enum order

    func testCmd1IsOverview() {
        let section = DashboardView.Section.allCases[0]
        XCTAssertEqual(section, .overview, "Section.allCases[0] must be .overview")
        XCTAssertEqual(
            shortcutMap["⌘1"], "Overview page",
            "⌘1 must be labeled 'Overview page' — Section.allCases[0] is .overview"
        )
    }

    func testCmd7IsHealthNotLogs() {
        // Section.allCases[6] == .health (the 7th section, 0-indexed).
        // The old HelpSheet incorrectly labeled ⌘7 as "Logs page".
        let section = DashboardView.Section.allCases[6]
        XCTAssertEqual(section, .health, "Section.allCases[6] must be .health")
        XCTAssertEqual(
            shortcutMap["⌘7"], "Health page",
            "⌘7 must be labeled 'Health page' — Section.allCases[6] is .health"
        )
    }

    func testCmd8IsLogs() {
        // Section.allCases[7] == .logs → ⌘8.
        // The old HelpSheet was missing ⌘8 entirely.
        let section = DashboardView.Section.allCases[7]
        XCTAssertEqual(section, .logs, "Section.allCases[7] must be .logs")
        XCTAssertEqual(
            shortcutMap["⌘8"], "Logs page",
            "⌘8 must be labeled 'Logs page' — Section.allCases[7] is .logs"
        )
    }

    // MARK: - AC: non-page shortcuts are present

    func testNonPageShortcutsPresent() {
        let map = shortcutMap
        let required: [(String, String)] = [
            ("⌘R", "Refresh all panels"),
            ("⌘K", "Open command palette"),
            ("⌘W", "Close window"),
            ("⌘,", "Open preferences"),
            ("⌘/", "Show this help"),
            ("esc", "Dismiss overlays"),
        ]
        for (key, expectedLabel) in required {
            XCTAssertEqual(
                map[key], expectedLabel,
                "HelpSheet must document \(key) as '\(expectedLabel)'"
            )
        }
    }

    // MARK: - AC: platform markers are macOS (⌘ not Ctrl)

    func testPlatformMarkersAreMacOS() {
        for shortcut in HelpSheet.shortcuts where shortcut.key != "esc" {
            XCTAssertTrue(
                shortcut.key.contains("⌘"),
                "Shortcut key '\(shortcut.key)' must use macOS ⌘ marker, not Ctrl or Super"
            )
        }
    }

    // MARK: - AC: total shortcut count

    func testTotalShortcutCount() {
        // 8 page shortcuts + ⌘R + ⌘K + ⌘W + ⌘, + ⌘/ + esc = 14
        XCTAssertEqual(
            HelpSheet.shortcuts.count, 14,
            "HelpSheet must document exactly 14 shortcuts (8 pages + 6 utility), found \(HelpSheet.shortcuts.count)"
        )
    }
}