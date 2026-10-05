import XCTest
import SwiftUI

@testable import ShareCLITray

/// PLAN.md:255-257 — task 1.29 (lane 9, P2):
/// "Tray: CTA tokens (lane 9, P2)
///  - Add `AccentColor` asset; replace `.buttonStyle(.borderedProminent)` calls
///    with `.tint(Color(\"AccentPrimary\"))`; add violet/green tokens from
///    `assets/tokens.css`."
///
/// FR: UNKNOWN — PLAN.md:255-257 names no FR/AC for task 1.29. Nearest honest
/// anchor is FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54), which
/// governs the tray surface as a whole but not design tokens. Recorded as
/// UNKNOWN rather than inventing an id.
///
/// Scope: the CTA token model consumed by the five owned files. The rendered
/// appearance of a tinted button is NOT assertable headlessly; what is asserted
/// is that the tokens exist, carry the tokens.css values, and that every CTA
/// site in the owned files resolves through them.
final class CtaTokenTests: XCTestCase {

    // MARK: - AC: tokens exist and carry the tokens.css values

    func testPrimaryAndSecondaryTokensExist() {
        // Presence check: these must resolve, not crash.
        _ = Color.ctaPrimary
        _ = Color.ctaSecondary
        _ = Color.tokenAccentColor
        _ = Color.tokenPulseGreen
        _ = Color.tokenSyncViolet
    }

    func testTokenHexValuesMatchTokensCss() {
        // assets/tokens.css :root (dark):
        //   --bb2-cta-primary:   #3fb950   (== --bb2-pulse-green)
        //   --bb2-cta-secondary: #a371f7   (== --bb2-sync-violet)
        XCTAssertEqual(CTATokens.primaryHex, 0x3FB950)
        XCTAssertEqual(CTATokens.secondaryHex, 0xA371F7)
        // Light appearance pair (tokens.css [data-theme="light"]):
        XCTAssertEqual(CTATokens.primaryLightHex, 0x1A7F37)
        XCTAssertEqual(CTATokens.secondaryLightHex, 0x8250DF)
    }

    func testGreenAndVioletTokensMatchTokensCss() {
        // --bb2-pulse-green: #3fb950 ; --bb2-sync-violet: #a371f7 (dark)
        XCTAssertEqual(CTATokens.pulseGreenHex, 0x3FB950)
        XCTAssertEqual(CTATokens.syncVioletHex, 0xA371F7)
    }

    func testAssetNameIsAccentColor() {
        // The plan names the asset "AccentColor"; its primary token is
        // "AccentPrimary".
        XCTAssertEqual(CTATokens.accentColorAssetName, "AccentColor")
        XCTAssertEqual(CTATokens.accentPrimaryAssetName, "AccentPrimary")
    }

    // MARK: - AC: hierarchy preserved (distinct button kinds stay distinct)

    func testCTAButtonKindStillRendersAsProminent() {
        // `.tint(...)` must be applied to a prominent button, not flattened to
        // a plain one: the plan replaces the *style tint*, not the emphasis.
        XCTAssertEqual(CTAButtonStyle.kind, .primaryCTA)
    }

    func testDestructiveKillKeepsItsOwnSemanticTint() {
        // AgentsPage's Kill PID button is destructive red, not the accent
        // primary; the rule must not flatten it into the CTA token.
        XCTAssertEqual(CTAButtonStyle.tint(for: .destructive), Color.red)
        XCTAssertEqual(CTAButtonStyle.tint(for: .primaryCTA), Color.ctaPrimary)
        XCTAssertEqual(CTAButtonStyle.tint(for: .secondaryCTA), Color.ctaSecondary)
    }

    // MARK: - AC: every CTA site in the owned files routes through the tokens

    private func ownedSource(_ name: String) throws -> String {
        let sourcesDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // ShareCLITrayTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // ShareCLITray
            .appendingPathComponent("Sources/ShareCLITray")
        return try String(contentsOf: sourcesDir.appendingPathComponent(name), encoding: .utf8)
    }

    func testOwnedFilesUseTheCTATokens() throws {
        for name in ["AgentsPage.swift", "TrayPopoverView.swift", "EmptyStateView.swift",
                     "UpdaterView.swift", "ChannelPicker.swift"] {
            let text = try ownedSource(name)
            XCTAssertTrue(
                text.contains("Color.ctaPrimary") || text.contains("Color.ctaSecondary")
                    || text.contains("CTAButtonStyle"),
                "\(name) does not resolve its CTA through the token model"
            )
        }
    }

    func testTrayPopoverAndUpdaterNoLongerRelyOnBareAccentColor() throws {
        // Before this task these sites let `.borderedProminent` pick up the
        // system accent. They must now pin the AccentPrimary token.
        for name in ["TrayPopoverView.swift", "UpdaterView.swift"] {
            let text = try ownedSource(name)
            XCTAssertTrue(
                text.contains(".tint(Color.ctaPrimary)"),
                "\(name) must tint its primary CTA with the AccentPrimary token"
            )
        }
    }
}
