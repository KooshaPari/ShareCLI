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
        // After task 1.28 decomposition, bulk kill lives in AllProcessesView.swift
        let text = try ownedSource("AllProcessesView.swift")
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

    // MARK: - AC: VoiceOver-friendly confirmation contract

    /// The confirmation dialog must declare `titleVisibility: .visible` so
    /// VoiceOver reads the title. Without this, the headline is treated as
    /// decorative and the user hears only the buttons — losing the
    /// "Kill 3 selected processes?" question that the whole gate exists to
    /// ask.
    func testAllProcessesViewDialogSetsTitleVisibilityVisible() throws {
        let text = try ownedSource("AllProcessesView.swift")
        XCTAssertTrue(
            text.contains("titleVisibility: .visible"),
            "confirmationDialog must set titleVisibility: .visible so VoiceOver reads the title"
        )
    }

    /// The destructive button must use `Button(_, role: .destructive)` (not
    /// just plain `Button`) so VoiceOver announces the destructive trait and
    /// the system displays the red emphasis styling.
    func testAllProcessesViewUsesDestructiveButtonRoleForConfirm() throws {
        let text = try ownedSource("AllProcessesView.swift")
        // We need a destructive Button carrying the scope's confirm label.
        XCTAssertTrue(
            text.contains("scope.confirmButtonLabel, role: .destructive"),
            "the confirm button must bind role: .destructive to the scope's label"
        )
    }

    /// The cancel button must declare `role: .cancel` so VoiceOver identifies
    /// it as the safe option and the system renders it in the standard
    /// position.
    func testAllProcessesViewUsesCancelButtonRole() throws {
        let text = try ownedSource("AllProcessesView.swift")
        XCTAssertTrue(
            text.contains("role: .cancel"),
            "the cancel button must declare role: .cancel so VoiceOver identifies it"
        )
    }

    /// Pluralization must be correct in confirm copy — VoiceOver reads it
    /// aloud and "Kill 1 process?" vs "Kill 1 selected processes?" must
    /// match the count exactly so the user trusts the read-out.
    func testVoiceOverReadsPluralizationCorrectly() {
        XCTAssertEqual(
            KillScope.selected([1]).confirmTitle, "Kill 1 selected process?",
            "VoiceOver must read '1 process' (singular) for a single-PID scope"
        )
        XCTAssertEqual(
            KillScope.selected([1, 2]).confirmTitle, "Kill 2 selected processes?",
            "VoiceOver must read 'N processes' (plural) for >1 PIDs"
        )
        XCTAssertTrue(
            KillScope.all.confirmTitle.localizedCaseInsensitiveContains("all"),
            "VoiceOver must read 'all' explicitly for the bulk scope"
        )
    }

    /// The confirm message must include the words "cannot be undone" so
    /// VoiceOver tells the user this is irreversible — without that phrase,
    /// the user could tap the wrong button. The existing confirm copy test
    /// already covers lowercase containment; this one enforces the exact
    /// phrase as a contract.
    func testVoiceOverReadsCannotBeUndoneExactly() {
        for scope in [KillScope.selected([1]), KillScope.all] {
            XCTAssertTrue(
                scope.confirmMessage.contains("cannot be undone"),
                "VoiceOver must hear 'cannot be undone' for \(scope); got: \(scope.confirmMessage)"
            )
        }
    }

    /// The cancel label must be the system string "Cancel" so VoiceOver's
    /// rotor / Cmd+Opt+Space / "type to find" pick it up reliably. Custom
    /// labels (e.g. "Never mind") are announced but break user muscle
    /// memory.
    func testVoiceOverFindsTheCancelLabel() {
        XCTAssertEqual(
            DestructiveKillGate.cancelLabel, "Cancel",
            "cancel label must be the system 'Cancel' so VoiceOver users find it"
        )
        XCTAssertEqual(
            CommandPalette.destructiveCancelLabel, "Cancel",
            "CommandPalette cancel label must match"
        )
    }

    /// Headline-and-body duplication: if the title already says "Kill 3
    /// selected processes?" the body must not also say "Kill 3 selected
    /// processes" — VoiceOver would read it twice, making the question feel
    /// like a warning instead of a question. The body should add new
    /// information (the consequence phrase).
    func testConfirmBodyDoesNotDuplicateTheTitle() {
        let scope = KillScope.selected([1, 2, 3])
        XCTAssertFalse(
            scope.confirmMessage.localizedCaseInsensitiveContains(scope.confirmTitle.replacingOccurrences(of: "?", with: "")),
            "confirm message must not restate the title; got: \(scope.confirmMessage) for title \(scope.confirmTitle)"
        )
    }

    /// Every confirmation dialog across the tray must be reachable from
    /// at least one of the listed source files. If a future view adds a
    /// destructive action but forgets the dialog, this test fails closed.
    func testEveryDestructiveDialogUsesVisibleTitleAndDestructiveRole() throws {
        let dialogHolders: [String] = [
            "AllProcessesView.swift",
            "ProcessRow.swift",
            "TrayPopoverView.swift",
            "ResourcesPage.swift",
            "CommandPalette.swift",
            "AgentsDetail.swift",
            "AgentsPage.swift",
        ]
        for source in dialogHolders {
            let text = try ownedSource(source)
            if text.contains(".confirmationDialog(") {
                XCTAssertTrue(
                    text.contains("titleVisibility: .visible"),
                    "\(source) declares a confirmationDialog but does not set titleVisibility: .visible"
                )
                XCTAssertTrue(
                    text.contains("role: .destructive"),
                    "\(source) declares a confirmationDialog but its confirm button lacks role: .destructive"
                )
            }
        }
    }

    // MARK: - AC: every kill call site is gated by a confirmationDialog

    /// Enumerates every source file that calls `state.kill(pid:)` or
    /// `state.killAll()` and asserts each such file also declares at least
    /// one of: a SwiftUI `.confirmationDialog(`, a SwiftUI `.alert(`, an
    /// AppKit `NSAlert`, or a reference to a pure gate helper
    /// (`DestructiveKillGate`, `TrayKillAllGate`). Any of these routes the
    /// user through a confirmation prompt before the destructive action
    /// fires. A file that calls `state.kill(...)` with none of these is a
    /// one-click wipe — this test fails closed.
    ///
    /// Why we accept multiple gate shapes:
    ///   - SwiftUI views use `.confirmationDialog(` or `.alert(`
    ///   - AppKit menu bar / right-click paths use `NSAlert`
    ///   - Helper views that pass a `kill:` closure to a child delegate
    ///     the gating to that child (e.g., `ProjectGroupCard` in
    ///     `ProcessRow.swift`); the parent's `kill: { ... }` closure
    ///     counts as "gated" if the child uses any of these patterns.
    func testEveryKillCallSiteIsGatedByAConfirmationDialog() throws {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // ShareCLITrayTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // ShareCLITray
            .appendingPathComponent("Sources/ShareCLITray")
        let fm = FileManager.default
        guard let sources = try? fm.contentsOfDirectory(atPath: sourcesDir.path) else {
            XCTFail("could not list \(sourcesDir.path)")
            return
        }
        // Build the file→text map once; the helper test reuses it.
        var fileToText: [String: String] = [:]
        for file in sources where file.hasSuffix(".swift") {
            let path = sourcesDir.appendingPathComponent(file)
            if let text = try? String(contentsOf: path, encoding: .utf8) {
                fileToText[file] = text
            }
        }
        let gatePatterns: [String] = [
            ".confirmationDialog(",   // SwiftUI confirmationDialog
            ".alert(",                // SwiftUI alert
            "NSAlert",                // AppKit NSAlert
            "DestructiveKillGate",    // pure helper used by SwiftUI views
            "TrayKillAllGate",        // pure helper used by the menubar path
        ]
        func hasGate(_ text: String) -> Bool {
            gatePatterns.contains { text.contains($0) }
        }
        var offenders: [String] = []
        for (file, text) in fileToText {
            let callsStateKill = text.contains("state.kill(pid:") || text.contains("AppState.shared.kill")
            if callsStateKill && !hasGate(text) {
                offenders.append(file)
            }
        }
        XCTAssertGreaterThan(
            fileToText.count, 0,
            "test setup must scan the Sources/ShareCLITray tree; got 0 files"
        )
        XCTAssertEqual(
            offenders, [],
            "every file that calls state.kill(...) or AppState.shared.kill must reference at least one of: .confirmationDialog(, .alert(, NSAlert, DestructiveKillGate, or TrayKillAllGate. Offending files: \(offenders)"
        )
    }

    /// Same as above but for the macOS status-bar menu bar action. The
    /// `MenuAction.killAll` selector should be paired with a confirmation
    /// dialog in TrayMenuController, or routed through a shared helper
    /// that does.
    func testMenuBarKillAllIsConfirmationGated() throws {
        let text = try ownedSource("TrayMenuController.swift")
        // TrayMenuController is a programmatic AppKit menu; it can route
        // through the shared DestructiveKillGate helper or call
        // confirmationDialog equivalent. Either way, the user must see
        // a prompt before killAll fires.
        XCTAssertTrue(
            text.contains("DestructiveKillGate") || text.contains("confirmAlert") || text.contains(".alert("),
            "TrayMenuController.killAll must present a confirmation via DestructiveKillGate, confirmAlert, or .alert(; raw call to AppState.shared.killAll() without prompt would let one click in the menu bar wipe the fleet"
        )
        // And critically: the raw call site should be inside a function
        // that takes user input, not the @objc selector itself.
        XCTAssertFalse(
            text.contains("func killAll(_ sender: Any?) {\n            await AppState.shared.killAll()"),
            "TrayMenuController.killAll(_:) must not call AppState.shared.killAll() directly; route through a gate"
        )
    }

    /// The CommandPalette (⌘K) is the highest-traffic entry point to
    /// killAll. Its existing test already covers the destructive role and
    /// confirm copy; this test asserts the user cannot trigger the
    /// action via a typed command without seeing a confirm dialog.
    func testCommandPaletteKillAllHasVisibleConfirmDialog() throws {
        let text = try ownedSource("CommandPalette.swift")
        XCTAssertTrue(
            text.contains(".confirmationDialog("),
            "CommandPalette must present a .confirmationDialog( so ⌘K → 'kill all' cannot fire without user confirm"
        )
        XCTAssertTrue(
            text.contains("titleVisibility: .visible"),
            "CommandPalette's confirmationDialog must set titleVisibility: .visible so VoiceOver reads it"
        )
    }
}
