import XCTest

@testable import ShareCLITray

/// PLAN.md:236-237 — task 1.24 (lane 9, P1):
/// "Tray: Destructive confirm + role (lane 9, P1)
///  - `Kill selected` / `Kill all` → `Button(role: .destructive)` +
///    `.confirmationDialog`."
///
/// FR: UNKNOWN — PLAN.md:236-237 names no FR/AC for task 1.24. Nearest honest
/// anchor is FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which
/// governs the tray surface as a whole but not destructive-action gating.
/// Recorded as UNKNOWN rather than inventing an id.
///
/// Scope: the *destructive-confirmation contract* for the tray's kill actions.
///   • ProcessesPage — `Kill selected` / `Kill all` bulk buttons.
///   • CommandPalette — the `Kill all processes` action entry.
///
/// The GUI click itself cannot be asserted headlessly; what is asserted is the
/// pure decision model (a kill scope cannot be performed without `confirm()`)
/// plus the SwiftUI role/copy contract the views are compile-bound to.
final class DestructiveActionConfirmTests: XCTestCase {

    // MARK: - AC: a kill scope cannot be performed without confirmation

    func testConfirmWithNoPendingRequestPerformsNothing() {
        var gate = DestructiveKillGate()
        XCTAssertNil(gate.pending)
        XCTAssertNil(gate.confirm(), "confirm() with no request must not yield a scope")
        XCTAssertNil(gate.pending)
    }

    func testRequestThenConfirmYieldsScopeAndClears() {
        var gate = DestructiveKillGate()
        gate.request(.all)
        XCTAssertTrue(gate.isConfirming)
        XCTAssertEqual(gate.confirm(), .all)
        XCTAssertNil(gate.pending, "confirm() must clear the pending scope")
        XCTAssertNil(gate.confirm(), "a second confirm() must not re-fire the same scope")
    }

    func testDeclineClearsPendingWithoutPerforming() {
        var gate = DestructiveKillGate()
        gate.request(.selected([42, 7]))
        XCTAssertTrue(gate.isConfirming)
        gate.decline()
        XCTAssertNil(gate.pending)
        XCTAssertNil(gate.confirm())
    }

    func testSelectedScopeCarriesExactlyTheRequestedPIDs() {
        var gate = DestructiveKillGate()
        gate.request(.selected([42, 7]))
        XCTAssertEqual(gate.confirm(), .selected([42, 7]))
    }

    func testRequestOverwritesPreviousIntent() {
        var gate = DestructiveKillGate()
        gate.request(.selected([1]))
        gate.request(.all)
        XCTAssertEqual(gate.pending, .all)
    }

    // MARK: - AC: actionable confirm copy for each scope

    func testConfirmCopyIsActionablePerScope() {
        let selected = KillScope.selected([1, 2, 3])
        XCTAssertEqual(selected.confirmTitle, "Kill 3 selected processes?")
        XCTAssertEqual(KillScope.selected([9]).confirmTitle, "Kill 1 selected process?")
        XCTAssertEqual(KillScope.all.confirmTitle, "Kill all processes?")
        XCTAssertEqual(selected.confirmButtonLabel, "Kill")
        XCTAssertEqual(KillScope.all.confirmButtonLabel, "Kill all")
        for scope in [selected, KillScope.all] {
            XCTAssertFalse(scope.confirmMessage.isEmpty)
            XCTAssertTrue(scope.confirmMessage.lowercased().contains("cannot be undone"))
        }
    }

    func testCancelLabelIsCancel() {
        XCTAssertEqual(DestructiveKillGate.cancelLabel, "Cancel")
    }

    // MARK: - AC: CommandPalette marks kill-all destructive + gates it

    func testPaletteKillAllIsDestructive() {
        XCTAssertTrue(CommandPalette.isDestructive(.killAll))
        XCTAssertEqual(CommandPalette.role(for: .killAll), .destructive)
    }

    func testPaletteNonDestructiveActionsHaveNoRole() {
        for action in [CommandPalette.CommandAction.refreshAll,
                       .exportProcessesJSON,
                       .exportProcessesCSV,
                       .clearFilter,
                       .showHelp,
                       .openLogFile,
                       .openPreferences] {
            XCTAssertFalse(CommandPalette.isDestructive(action), "\(action) must not be destructive")
            XCTAssertNil(CommandPalette.role(for: action))
        }
    }

    func testPaletteConfirmCopyIsActionable() {
        XCTAssertEqual(CommandPalette.destructiveConfirmTitle, "Kill all processes?")
        XCTAssertEqual(CommandPalette.destructiveConfirmLabel, "Kill all")
        XCTAssertEqual(CommandPalette.destructiveCancelLabel, "Cancel")
        XCTAssertTrue(CommandPalette.destructiveConfirmMessage.lowercased().contains("cannot be undone"))
    }

    // MARK: - AC: the views wire role + confirmationDialog

    private func ownedSource(_ name: String) throws -> String {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // ShareCLITrayTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // ShareCLITray
            .appendingPathComponent("Sources/ShareCLITray")
        return try String(contentsOf: sourcesDir.appendingPathComponent(name), encoding: .utf8)
    }

    func testProcessesPageKillButtonsAreDestructiveAndConfirmed() throws {
        let text = try ownedSource("ProcessesPage.swift")
        XCTAssertTrue(text.contains("Button(role: .destructive)"),
                      "bulk kill buttons must declare role: .destructive")
        XCTAssertTrue(text.contains(".confirmationDialog("),
                      "ProcessesPage must present a confirmationDialog")
        XCTAssertTrue(text.contains("Kill selected"),
                      "the named Kill selected affordance must survive")
        XCTAssertTrue(text.contains("Kill all"))
    }

    func testCommandPaletteGatesKillAllWithAConfirmationDialog() throws {
        let text = try ownedSource("CommandPalette.swift")
        XCTAssertTrue(text.contains(".confirmationDialog("),
                      "CommandPalette must present a confirmationDialog for kill-all")
        XCTAssertTrue(text.contains("isDestructive"))
        XCTAssertFalse(text.contains("state.killAll()"),
                       "the palette must not call killAll directly; it routes through onAction after confirm")
    }
}
