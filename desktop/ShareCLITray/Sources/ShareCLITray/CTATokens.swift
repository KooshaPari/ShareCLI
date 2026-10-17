/// CTATokens.swift — CTA colour tokens for the tray, mirroring `assets/tokens.css`.
///
/// PLAN.md:255-257 — task 1.29 (lane 9, P2):
/// "Tray: CTA tokens (lane 9, P2)
///  - Add `AccentColor` asset; replace `.buttonStyle(.borderedProminent)` calls
///    with `.tint(Color(\"AccentPrimary\"))`; add violet/green tokens from
///    `assets/tokens.css`."
///
/// Token values (verbatim from `assets/tokens.css`):
///
///   dark  (`:root`)            light (`[data-theme="light"]` / prefers light)
///   --bb2-cta-primary:   #3fb950   --bb2-cta-primary:   #1a7f37
///   --bb2-cta-secondary: #a371f7   --bb2-cta-secondary: #8250df
///   --bb2-pulse-green:   #3fb950   --bb2-pulse-green:   #1a7f37
///   --bb2-sync-violet:   #a371f7   --bb2-sync-violet:   #8250df
///
/// Asset name: `AccentColor` (the plan's named asset); its primary CTA token is
/// `AccentPrimary`. Both resolve from the bundled `Assets.xcassets` catalog and
/// fall back to the literal tokens.css values when the catalog is not present
/// in a headless build, so the token is never an unresolved colour.
///
/// House style follows `SemanticColors.swift` (extension on `Color` + a small
/// pure namespace), so the decision surface stays unit-testable.

import SwiftUI

/// Pure token definitions: the values a test can assert without a render.
enum CTATokens {
    // Asset names.
    static let accentColorAssetName = "AccentColor"
    static let accentPrimaryAssetName = "AccentPrimary"
    static let pulseGreenAssetName = "PulseGreen"
    static let syncVioletAssetName = "SyncViolet"

    // tokens.css values, as literals (dark appearance).
    static let primaryHex: UInt32 = 0x3F_B950
    static let secondaryHex: UInt32 = 0xA3_71F7
    static let pulseGreenHex: UInt32 = 0x3F_B950
    static let syncVioletHex: UInt32 = 0xA3_71F7

    // tokens.css light-appearance pair.
    static let primaryLightHex: UInt32 = 0x1A_7F37
    static let secondaryLightHex: UInt32 = 0x82_50DF

    /// Build a `Color` for a tokens.css value, preferring the asset catalog and
    /// falling back to the literal when the asset is unbundled.
    static func resolved(_ assetName: String, fallback hex: UInt32) -> Color {
        #if canImport(AppKit)
        if NSColor(named: assetName) != nil {
            return Color(assetName)
        }
        #endif
        return Color(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255.0,
            green: Double((hex >> 8) & 0xFF) / 255.0,
            blue: Double(hex & 0xFF) / 255.0,
            opacity: 1.0
        )
    }
}

extension Color {
    /// The plan's named asset, resolved.
    static var tokenAccentColor: Color {
        CTATokens.resolved(CTATokens.accentColorAssetName, fallback: CTATokens.primaryHex)
    }

    /// Primary CTA token (`--bb2-cta-primary`, green).
    static var ctaPrimary: Color {
        CTATokens.resolved(CTATokens.accentPrimaryAssetName, fallback: CTATokens.primaryHex)
    }

    /// Secondary CTA token (`--bb2-cta-secondary`, violet).
    static var ctaSecondary: Color {
        CTATokens.resolved(CTATokens.secondaryHexName, fallback: CTATokens.secondaryHex)
    }

    /// `--bb2-pulse-green`.
    static var tokenPulseGreen: Color {
        CTATokens.resolved(CTATokens.pulseGreenAssetName, fallback: CTATokens.pulseGreenHex)
    }

    /// `--bb2-sync-violet`.
    static var tokenSyncViolet: Color {
        CTATokens.resolved(CTATokens.syncVioletAssetName, fallback: CTATokens.syncVioletHex)
    }
}

extension CTATokens {
    /// Asset name backing `Color.ctaSecondary`.
    static var secondaryHexName: String { syncVioletAssetName }
}

/// Which CTA a button is, so the accent tint does not flatten distinct kinds.
enum CTAButtonKind {
    /// Accent-coloured primary action (`--bb2-cta-primary`).
    case primaryCTA
    /// Secondary accent action (`--bb2-cta-secondary`).
    case secondaryCTA
    /// Destructive action keeps its semantic red (e.g. Kill PID).
    case destructive
}

enum CTAButtonStyle {
    /// The emphasis the plan keeps: tinting a *prominent* button, not
    /// downgrading it to a plain one.
    static let kind = CTAButtonKind.primaryCTA

    /// Tint per kind. Destructive stays red so the kill affordance is not
    /// recoloured by the CTA token sweep.
    static func tint(for kind: CTAButtonKind) -> Color {
        switch kind {
        case .primaryCTA: return .ctaPrimary
        case .secondaryCTA: return .ctaSecondary
        case .destructive: return .red
        }
    }
}
