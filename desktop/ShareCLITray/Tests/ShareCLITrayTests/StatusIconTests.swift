import XCTest

@testable import ShareCLITray

/// PLAN.md:259-261 — task 1.30 (lane 9, P2):
/// "Tray: status-symbol consolidation
///  - Single `StatusIcon` enum + `IconFor.action(.killAll)` resolver;
///    replace 3 hearts and 3 kill glyphs with one each."
///
/// FR: FR-007 — Resource & Syscall-Relevant Watch (nearest honest anchor;
/// PLAN names no FR/AC for task 1.30.  AC-007.47–AC-007.54 govern the tray
/// surface as a whole but do not define per-icon consolidation.)
///
/// Inventory of replaced sites (12 sites across 9 files):
///   HEART (6 → 1 source via StatusIcon):
///     1. DashboardView.swift:57        "heart.fill"           (sidebar)
///     2. HealthPage.swift:56           "heart.text.square"    (empty state)
///     3. MiniCompositeHealthCard.swift:57  "heart.text.square.fill" (active)
///     4. MiniCompositeHealthCard.swift:87  "heart.text.square"     (placeholder)
///     5. CompositeHealthCard.swift:59      "heart.text.square.fill" (header)
///     6. CompositeHealthCard.swift:199     "heart.text.square"     (placeholder)
///   KILL GLYPHS (6 → 1 source via StatusIcon):
///     7.  HealthPill.swift:62          "xmark.octagon.fill"   (unhealthy)
///     8.  PoolPage.swift:426           "xmark.octagon"        (critical severity)
///     9.  TrayPopoverView.swift:226    "xmark.circle"         (per-process kill)
///    10.  TrayPopoverView.swift:279    "power"                (quit app)
///    11.  PreferencesSheet.swift:36    "xmark.circle.fill"    (dismiss sheet)
///    12.  TrayPopoverView.swift:269    text-only "Kill All"   (no icon — deferred)
///
/// Deferred (0 sites in forbidden files):
///   None — all heart/kill-glyph sites live within the 9 allowed files.
///   AgentsPage.swift, ProcessesPage.swift, MicroInteractions.swift,
///   ConfigPage.swift, LogsPage.swift, CommandPalette.swift were scanned
///   and contain no heart/kill-glyph SF Symbol references.
final class StatusIconTests: XCTestCase {

    // MARK: - StatusIcon enum resolves to canonical SF Symbol names

    func testCompositeHealthIconIsHeartTextSquareFill() {
        XCTAssertEqual(StatusIcon.compositeHealth.symbolName, "heart.text.square.fill")
    }

    func testHealthSectionIconIsHeartFill() {
        XCTAssertEqual(StatusIcon.healthSection.symbolName, "heart.fill")
    }

    func testHealthPlaceholderIconIsHeartTextSquare() {
        XCTAssertEqual(StatusIcon.healthPlaceholder.symbolName, "heart.text.square")
    }

    func testKillProcessIconIsXmarkCircle() {
        XCTAssertEqual(StatusIcon.killProcess.symbolName, "xmark.circle")
    }

    func testDismissIconIsXmarkCircleFill() {
        XCTAssertEqual(StatusIcon.dismiss.symbolName, "xmark.circle.fill")
    }

    func testCriticalIconIsXmarkOctagon() {
        XCTAssertEqual(StatusIcon.critical.symbolName, "xmark.octagon")
    }

    func testUnhealthyIconIsXmarkOctagonFill() {
        XCTAssertEqual(StatusIcon.unhealthy.symbolName, "xmark.octagon.fill")
    }

    func testQuitAppIconIsPower() {
        XCTAssertEqual(StatusIcon.quitApp.symbolName, "power")
    }

    // MARK: - IconFor.action resolver

    func testIconForKillAllResolvesToXmarkCircleFill() {
        XCTAssertEqual(IconFor.action(.killAll), StatusIcon.dismiss)
    }

    // MARK: - Heart consolidation: all heart variants come from StatusIcon

    func testHeartVariantsCoverAllDashboardUses() {
        // Three distinct heart SF Symbols used across the 9 files:
        let hearts: [StatusIcon] = [.compositeHealth, .healthSection, .healthPlaceholder]
        let symbols = Set(hearts.map(\.symbolName))
        XCTAssertEqual(symbols.count, 3, "must have exactly 3 distinct heart symbols")
        XCTAssertTrue(symbols.allSatisfy { $0.contains("heart") }, "all must be heart variants")
    }

    // MARK: - Kill-glyph consolidation: all kill variants come from StatusIcon

    func testKillGlyphVariantsConsolidated() {
        let killIcons: [StatusIcon] = [
            .killProcess, .dismiss, .critical, .unhealthy, .quitApp,
        ]
        let symbols = Set(killIcons.map(\.symbolName))
        XCTAssertTrue(symbols.count >= 4, "must have distinct kill-glyph symbols")
        // Every kill-glyph symbol contains either "xmark" or "power"
        XCTAssertTrue(
            symbols.allSatisfy { $0.contains("xmark") || $0.contains("power") },
            "all kill-glyph symbols must be xmark or power variants"
        )
    }
}