import XCTest
import SwiftUI

@testable import ShareCLITray

/// PLAN.md:239-240 — closure batch (turtle, 2026-10-05) — three deferred sites
/// from tasks 1.24, 1.25, 1.29:
///   (A1) borderedProminent remnants in the split-out process views
///        (SpawnForm.swift, ResourcesPage.swift) → CTA-token tint per 1.29.
///   (A2) kill-confirm gaps: per-row kill in AllProcessesView (`Table` Actions)
///        and ResourcesPage / ProcessRowInline kill buttons must be gated by
///        `DestructiveKillGate`; TrayPopoverView + TrayMenuController kill-all
///        must be confirmed.
///   (A3) ⌘F palette search field (4th filter field deferred by 1.25) must be
///        wired through the existing `FilterFieldShortcut` contract
///        (`filterFieldFocus`), and moved out of `outOfScopeFields`.
///
/// FR: FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54). PLAN names no
/// FR/AC id for the closure batch; FR-007 governs the tray surface as a whole
/// and is the nearest honest anchor. Recorded as the anchor, not an invented id.
///
/// The GUI click / focus itself is NOT assertable headlessly: asserted here are
/// the pure decisions (gate state machine, CTA kind→tint, shortcut contract) and
/// the source-level wiring invariants.
final class ClosureKillCmdFTests: XCTestCase {

    // MARK: - A1: CTA token tint on the split-out prominent buttons

    func testPrimaryCTAStillUsesThePrimaryToken() {
        XCTAssertEqual(CTAButtonStyle.kind, .primaryCTA)
        XCTAssertEqual(CTAButtonStyle.tint(for: .primaryCTA), Color.ctaPrimary)
        XCTAssertEqual(CTAButtonStyle.tint(for: .secondaryCTA), Color.ctaSecondary)
        XCTAssertEqual(CTAButtonStyle.tint(for: .destructive), Color.red)
    }

    // MARK: - A2: kill gate covers per-row kills

    func testPerRowKillScopeIsGated() {
        // A per-row kill must be requested through the gate, not performed
        // directly: request then confirm yields exactly that PID.
        var gate = DestructiveKillGate()
        gate.request(.selected([4242]))
        XCTAssertTrue(gate.isConfirming)
        XCTAssertEqual(gate.confirm(), .selected([4242]))
        XCTAssertNil(gate.confirm(), "a per-row kill must not fire twice")
    }

    func testSinglePIDTitleIsSingular() {
        XCTAssertEqual(KillScope.selected([7]).confirmTitle, "Kill 1 selected process?")
        XCTAssertEqual(KillScope.selected([7, 8]).confirmTitle, "Kill 2 selected processes?")
    }

    // MARK: - A2: TrayMenuController confirmation decision

    func testMenuBarKillAllCannotFireWithoutConfirmation() {
        XCTAssertNil(TrayKillAllGate.confirm(pending: false))
        XCTAssertEqual(TrayKillAllGate.confirm(pending: true), .all)
    }

    // MARK: - A3: ⌘F palette search field is wired, not deferred

    func testPaletteSearchIsNoLongerOutOfScope() {
        XCTAssertTrue(
            FilterFieldShortcut.wiredFields.contains(.paletteSearch),
            "the 4th filter field (⌘K palette search) is now wired by the closure batch"
        )
        XCTAssertFalse(
            FilterFieldShortcut.outOfScopeFields.contains(.paletteSearch),
            "palette search must no longer be recorded as deferred"
        )
        XCTAssertEqual(FilterFieldShortcut.wiredFields.count, 4)
    }

    func testShortcutKeyAndModifiersAreTheReusedContract() {
        XCTAssertEqual(FilterFieldShortcut.key, "f")
        XCTAssertEqual(FilterFieldShortcut.modifiers, .command)
        XCTAssertFalse(FilterFieldShortcut.collidesWithReserved("f"),
                       "⌘F must not collide with a reserved dashboard shortcut")
    }

    // MARK: - Source invariants

    private func source(_ name: String) throws -> String {
        let dir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Sources/ShareCLITray")
        return try String(contentsOf: dir.appendingPathComponent(name), encoding: .utf8)
    }

    func testPaletteSearchFieldCarriesTheShortcutModifier() throws {
        let text = try source("CommandPalette.swift")
        XCTAssertTrue(text.contains("filterFieldFocus("),
                      "the palette search field must reuse the FilterFieldShortcut contract")
    }

    func testSplitProcessViewsTintTheirProminentButtons() throws {
        for name in ["SpawnForm.swift", "ResourcesPage.swift"] {
            let text = try source(name)
            XCTAssertTrue(
                text.contains("Color.ctaPrimary") || text.contains("CTAButtonStyle"),
                "\(name) must resolve its prominent button tint through the CTA tokens"
            )
        }
    }

    func testMenuControllerKillAllIsConfirmed() throws {
        let text = try source("TrayMenuController.swift")
        XCTAssertTrue(text.contains("TrayKillAllGate"),
                      "TrayMenuController must gate kill-all through the confirmation decision")
    }
}
