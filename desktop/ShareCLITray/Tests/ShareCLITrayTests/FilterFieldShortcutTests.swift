import SwiftUI
import XCTest

@testable import ShareCLITray

/// PLAN.md:239-240 — task 1.25 (lane 9, P1):
/// "Tray: ⌘F filter focus (lane 9, P1)
///  - Four filter fields get a `.keyboardShortcut("f", modifiers: .command)`."
///
/// FR: FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54). PLAN.md
/// names no FR/AC id for task 1.25; FR-007 governs the tray surface as a whole
/// and is the same honest anchor task 1.21 used. Recorded as FR-007 rather than
/// inventing an id.
///
/// Scope: the tray's ⌘F contract.
///   1. The filter shortcut is exactly ⌘F (`"f"` + `.command`), so the HelpSheet
///      row documented by task 1.26 is truthful.
///   2. It collides with no app-global shortcut already owned by
///      `DashboardView.attachShortcutMonitor` (⌘1..⌘8, ⌘K, ⌘R, ⌘W, ⌘/, ⌘,).
///   3. Every filter-bearing page is registered. The app has FOUR
///      filter/search text fields; the fourth (the ⌘K palette's search field in
///      `CommandPalette.swift`) was originally deferred to lane 1.20 by task
///      1.25 and is now wired by the closure batch (cricket 2026-10-05, A3), so
///      all four fields are in `wiredFields` and none remains deferred.
///
/// The SwiftUI focus wiring (`@FocusState` + the shortcut trigger) is
/// compile-verified by the `ShareCLITray` target. End-to-end GUI focus
/// (⌘F → caret in the field) cannot be observed in a headless XCTest run and is
/// therefore not asserted; the pure contract the wiring depends on is.
final class FilterFieldShortcutTests: XCTestCase {

    // MARK: - Contract 1: the shortcut is exactly ⌘F

    func testShortcutKeyIsF() {
        XCTAssertEqual(
            FilterFieldShortcut.key, "f",
            "the filter focus shortcut must be the F key"
        )
    }

    func testShortcutModifierIsCommandOnly() {
        XCTAssertEqual(
            FilterFieldShortcut.modifiers, .command,
            "⌘F must be Command-only; any Shift/Option/Control variant would not match the documented shortcut"
        )
    }

    // MARK: - Contract 2: no collision with existing global shortcuts

    func testShortcutDoesNotCollideWithReservedGlobals() {
        XCTAssertFalse(
            FilterFieldShortcut.collidesWithReserved(FilterFieldShortcut.key),
            "⌘F must not collide with a DashboardView global shortcut"
        )
    }

    func testReservedGlobalsMatchTheDashboardMonitor() {
        // Mirror of DashboardView.attachShortcutMonitor: ⌘1..⌘8 then k/r/w///,.
        let expected: [KeyEquivalent] = ["1", "2", "3", "4", "5", "6", "7", "8", "k", "r", "w", "/", ","]
        XCTAssertEqual(
            Set(FilterFieldShortcut.reservedGlobalShortcuts.map(\.character)),
            Set(expected.map(\.character)),
            "the reserved set must track the actual monitor so a future ⌘F wiring cannot be checked against a stale list"
        )
    }

    // MARK: - Contract 3: every in-scope filter field is registered

    func testWiredFieldsCoverAllFourFilterPages() {
        // Closure batch (cricket 2026-10-05, A3): the ⌘K palette search field,
        // originally deferred to lane 1.20 by task 1.25, is now wired, so all
        // four filter fields across the app carry the ⌘F shortcut.
        XCTAssertEqual(
            FilterFieldShortcut.wiredFields.map(\.rawValue),
            ["processes.filter", "agents.filter", "logs.filter", "palette.search"],
            "all four filter fields are Processes / Agents / Logs / palette search"
        )
    }

    func testNoFilterFieldRemainsDeferred() {
        XCTAssertEqual(
            FilterFieldShortcut.outOfScopeFields.map(\.rawValue),
            [],
            "the closure batch wired the last deferred field (palette search); nothing remains out of scope"
        )
    }

    func testAllFieldsAreDistinct() {
        let all = FilterFieldShortcut.Field.allCases.map(\.rawValue)
        XCTAssertEqual(
            Set(all).count, all.count,
            "each filter field must have a unique identity"
        )
    }
}
