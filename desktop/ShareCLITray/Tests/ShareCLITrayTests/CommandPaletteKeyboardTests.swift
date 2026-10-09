import SwiftUI
import XCTest

@testable import ShareCLITray

/// PLAN.md:220-222 — task 1.20 (lane 9, P0):
/// "Tray: Cmd+K palette kbd nav + focus trap —
///  `CommandPalette.swift` — `FocusState`, `onSubmit`,
///  `onKeyPress(.upArrow/.downArrow)`; palette `.isModal` to AT."
///
/// FR: UNKNOWN — PLAN.md names no FR/AC for task 1.20. Nearest honest anchor is
/// FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which governs the
/// tray surface as a whole but not keyboard or accessibility behaviour. Recorded
/// here as UNKNOWN rather than inventing an id.
///
/// Scope: the palette's *keyboard contract* (selection arithmetic, key→intent
/// mapping, modal accessibility trait). The SwiftUI modifier wiring
/// (`FocusState` / `onSubmit` / `onKeyPress`) is compile-verified by this
/// target building `ShareCLITray`; the observable behaviour it drives is the
/// pure logic asserted below.
final class CommandPaletteKeyboardTests: XCTestCase {

    // MARK: - AC: up/down arrow moves the highlighted option

    func testArrowDownAdvancesTheHighlight() {
        XCTAssertEqual(CommandPalette.moveDown(from: 0, count: 3), 1)
        XCTAssertEqual(CommandPalette.moveDown(from: 1, count: 3), 2)
    }

    func testArrowDownStopsAtTheLastOption() {
        XCTAssertEqual(
            CommandPalette.moveDown(from: 2, count: 3), 2,
            "highlight must not run past the final option"
        )
    }

    func testArrowUpRetreatsTheHighlight() {
        XCTAssertEqual(CommandPalette.moveUp(from: 2), 1)
        XCTAssertEqual(CommandPalette.moveUp(from: 1), 0)
    }

    func testArrowUpStopsAtTheFirstOption() {
        XCTAssertEqual(
            CommandPalette.moveUp(from: 0), 0,
            "highlight must not run above the first option"
        )
    }

    func testArrowKeysAreRecognisedAsSelectionMoves() {
        XCTAssertEqual(CommandPalette.intent(for: .downArrow), .moveDown)
        XCTAssertEqual(CommandPalette.intent(for: .upArrow), .moveUp)
    }

    // MARK: - AC: Enter submits the highlighted option

    func testEnterIsRecognisedAsSubmit() {
        XCTAssertEqual(CommandPalette.intent(for: .return), .submit)
    }

    func testSubmitTargetIsTheHighlightedOption() {
        let entries = ["overview", "processes", "agents"]
        let highlighted = CommandPalette.moveDown(from: 0, count: entries.count)
        XCTAssertEqual(
            entries[CommandPalette.resolvedIndex(highlighted, count: entries.count)],
            "processes",
            "Enter must submit the option the highlight is on, not the first one"
        )
    }

    func testSubmitTargetIsSafeWhenTheListIsEmpty() {
        XCTAssertEqual(CommandPalette.resolvedIndex(0, count: 0), 0)
        XCTAssertEqual(CommandPalette.resolvedIndex(7, count: 0), 0)
    }

    func testSubmitTargetClampsAStaleIndex() {
        XCTAssertEqual(
            CommandPalette.resolvedIndex(9, count: 3), 2,
            "a stale highlight from a longer result list must clamp, not crash"
        )
    }

    // MARK: - AC: Tab cannot escape the palette while open (focus trap)

    func testTabIsTrapped() {
        XCTAssertEqual(CommandPalette.intent(for: .tab), .trapTab)
    }

    func testTabDoesNotDismissThePalette() {
        XCTAssertNotEqual(
            CommandPalette.intent(for: .tab), .dismiss,
            "Tab must be swallowed; only Escape may close the palette"
        )
    }

    func testEscapeIsTheOnlyDismissIntent() {
        let keys: [KeyEquivalent] = [.tab, .upArrow, .downArrow, .return, .space, .delete]
        for key in keys {
            XCTAssertNotEqual(
                CommandPalette.intent(for: key), .dismiss,
                "\(key) must not close the palette"
            )
        }
        XCTAssertEqual(CommandPalette.intent(for: .escape), .dismiss)
    }

    func testUnrecognisedKeysPassThroughToTheSearchField() {
        XCTAssertEqual(
            CommandPalette.intent(for: "a"), .passThrough,
            "ordinary typing must reach the text field rather than be swallowed"
        )
    }

    // MARK: - AC: palette is exposed as modal to accessibility

    func testPaletteDeclaresModalAccessibilityTrait() {
        XCTAssertTrue(
            CommandPalette.modalAccessibilityTraits.contains(.isModal),
            "the palette container must advertise .isModal so VoiceOver traps focus"
        )
    }
}
