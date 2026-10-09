import XCTest
import SwiftUI

@testable import ShareCLITray

/// PLAN.md:233-234 — task 1.23 (lane 9, P1):
/// "Tray: Reduce-motion honored (lane 9, P1)
///  - Wrap every motion site in `@Environment(\.accessibilityReduceMotion)`."
///
/// FR: UNKNOWN — PLAN.md:233-234 names no FR/AC for task 1.23. Nearest honest
/// anchor is FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which
/// governs the tray surface as a whole but not motion accessibility. Recorded
/// as UNKNOWN rather than inventing an id.
///
/// Scope: the reduce-motion decision used by every motion site in the five
/// owned files (AgentsPage, MicroInteractions, ConfigPage, LogsPage,
/// ProcessesPage).
///
/// The GUI animation itself is NOT assertable headlessly. What is asserted is
/// the pure decision function — a motion site animates if and only if reduce
/// motion is off — plus the source-level invariant that every animation entry
/// point is routed through that decision rather than a bare `withAnimation`.
final class ReduceMotionTests: XCTestCase {

    // MARK: - AC: motion is gated on reduceMotion

    func testMotionIsSuppressedWhenReduceMotionIsOn() {
        XCTAssertFalse(Motion.isEnabled(reduceMotion: true),
                       "reduce motion on must disable motion")
        XCTAssertTrue(Motion.isEnabled(reduceMotion: false),
                      "reduce motion off must keep motion")
    }

    func testAnimationDecisionReturnsNilWhenReduced() {
        let spring = Animation.spring(response: 0.42, dampingFraction: 0.78)
        XCTAssertNil(Motion.animation(spring, reduceMotion: true))
        XCTAssertNotNil(Motion.animation(spring, reduceMotion: false))
    }

    func testDecisionMatchesEnvironmentInEitherDirection() {
        // Exhaustive over the 2-valued environment; no invented expectations.
        for reduced in [true, false] {
            XCTAssertEqual(Motion.isEnabled(reduceMotion: reduced), !reduced)
        }
    }

    // MARK: - AC: state changes still apply when motion is suppressed

    func testRunStillPerformsStateChangeWhenReduced() {
        // "no movement" must not mean "no state change": a toggle must still
        // toggle, just instantly. Verified via a captured side-effect counter.
        var applied = 0
        Motion.run(.spring(), reduceMotion: true) { applied += 1 }
        XCTAssertEqual(applied, 1, "state change must still run without animation")

        var applied2 = 0
        Motion.run(.spring(), reduceMotion: false) { applied2 += 1 }
        XCTAssertEqual(applied2, 1)
    }

    func testRunWithNilAnimationStillApplies() {
        var applied = 0
        Motion.run(nil, reduceMotion: false) { applied += 1 }
        XCTAssertEqual(applied, 1)
    }

    // MARK: - AC: source invariant — no bare withAnimation entry point remains

    private func ownedSource(_ name: String) throws -> String {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // ShareCLITrayTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // ShareCLITray
            .appendingPathComponent("Sources/ShareCLITray")
        return try String(contentsOf: sourcesDir.appendingPathComponent(name), encoding: .utf8)
    }

    func testOwnedFilesDoNotCallWithAnimationDirectly() throws {
        // Every motion site must route through Motion.run / Motion.animation so
        // the reduce-motion decision is applied. The one sanctioned
        // `withAnimation` call is inside Motion.run itself, which consumes the
        // already-gated animation; any other occurrence means a site was missed.
        let sanctioned = "withAnimation(animation, change)"
        for name in ["AgentsPage.swift", "MicroInteractions.swift",
                     "ConfigPage.swift", "LogsPage.swift", "ProcessesPage.swift"] {
            let text = try ownedSource(name)
            let stripped = text.replacingOccurrences(of: sanctioned, with: "")
            XCTAssertFalse(
                stripped.contains("withAnimation("),
                "\(name) calls withAnimation outside Motion.run"
            )
        }
    }

    func testMotionModifiersReadTheEnvironmentFlag() throws {
        let text = try ownedSource("MicroInteractions.swift")
        XCTAssertTrue(
            text.contains("@Environment(\\.accessibilityReduceMotion)"),
            "motion modifiers must read accessibilityReduceMotion from the environment"
        )
    }
}
