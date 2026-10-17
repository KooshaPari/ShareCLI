/// SemanticColors.swift — WCAG-AA semantic colour tokens for tray text.
///
/// PLAN.md:245-247 — task 1.27 (lane 9, P1):
/// "Audit 8 files for status text contrast; introduce semantic tokens
///  (`StatusForeground`, `ValueForeground`) and check both appearances."
///
/// Problem: `.tertiary` (≈30% primary opacity) fails WCAG AA 4.5:1 for body
/// text in both light (~2.8:1) and dark (~3.0:1) appearances.  `.secondary`
/// (≈60% opacity) passes AA in both (~4.6:1 light, ~4.5:1 dark).
///
/// Solution: two semantic tokens that guarantee AA compliance in both
/// appearances, replacing raw `.tertiary` callsites for status text.

import SwiftUI

extension Color {
    /// Semantic token: status/label foreground.
    ///
    /// Use for status text, captions, subtitles, and descriptive labels
    /// that must meet WCAG AA 4.5:1 in both light and dark appearances.
    ///
    /// Resolves to:
    /// - Light appearance: NSColor.secondaryLabelColor (~60% opacity, ~4.6:1)
    /// - Dark appearance:  NSColor.secondaryLabelColor (~60% opacity, ~4.5:1)
    ///
    /// Replaces `.tertiary` where it was used for body-sized status text.
    static var statusForeground: Color {
        .secondary
    }

    /// Semantic token: value/data foreground.
    ///
    /// Use for numeric values, metric readouts, and primary data points
    /// that must be immediately legible.
    ///
    /// Resolves to:
    /// - Light appearance: NSColor.labelColor (100% opacity, ~21:1)
    /// - Dark appearance:  NSColor.labelColor (100% opacity, ~21:1)
    static var valueForeground: Color {
        .primary
    }

    /// Semantic token: warning/alert foreground (orange status indicators).
    ///
    /// Use for warning badges, alert text, and caution indicators that must
    /// meet WCAG AA 4.5:1 in both appearances.
    ///
    /// Resolves to:
    /// - Light appearance: dark amber #994C00 (rendered RGB 0.60, 0.30, 0.00; 6.15:1 on white)
    /// - Dark appearance:  SwiftUI .orange (rendered RGB 1.00, 0.55, 0.16; 7.57:1 on near-black)
    ///
    /// Ratios are pixel-rendered via `SemanticColorRenderTests`
    /// (SwiftUI `ImageRenderer` + WCAG 2.1 luminance), not hand-computed.
    static var warningForeground: Color {
        warningForeground(isDark: Self.isDarkMode)
    }

    /// Pure-function variant: takes the appearance as a parameter so
    /// unit tests can exercise both light and dark branches without
    /// having to mutate `NSApp.appearance` (which `ImageRenderer` does
    /// not honor inside an XCTest runner).
    static func warningForeground(isDark: Bool) -> Color {
        if isDark { return .orange }
        return Color(red: 0.60, green: 0.30, blue: 0.0)
    }

    /// Semantic token: success/healthy foreground (green status indicators).
    ///
    /// Use for success badges, healthy-state text, and positive indicators
    /// that must meet WCAG AA 4.5:1 in both appearances.
    ///
    /// Resolves to:
    /// - Light appearance: dark green #1A6B1A (rendered RGB 0.10, 0.42, 0.10; 6.65:1 on white)
    /// - Dark appearance:  SwiftUI .green (rendered RGB 0.20, 0.78, 0.35; 7.88:1 on near-black)
    ///
    /// Ratios are pixel-rendered via `SemanticColorRenderTests`
    /// (SwiftUI `ImageRenderer` + WCAG 2.1 luminance), not hand-computed.
    static var successForeground: Color {
        successForeground(isDark: Self.isDarkMode)
    }

    /// Pure-function variant of `successForeground`. See
    /// `warningForeground(isDark:)` for the rationale.
    static func successForeground(isDark: Bool) -> Color {
        if isDark { return .green }
        return Color(red: 0.10, green: 0.42, blue: 0.10)
    }

    /// Whether the running app is in dark mode. The default
    /// implementation consults `NSApp?.effectiveAppearance`; tests can
    /// override this via `isDarkModeOverride` to drive the static
    /// `warningForeground` / `successForeground` accessors through a
    /// specific branch.
    private static var isDarkMode: Bool {
        if let override = isDarkModeOverride { return override }
        return NSApp?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
    }

    /// Test seam: when set, `warningForeground` and `successForeground`
    /// ignore `NSApp.appearance` and use this value instead. Used by
    /// `SemanticColorRenderTests` to exercise both branches from a
    /// single XCTest process (the runner defaults to dark and
    /// `ImageRenderer` does not honor a runtime appearance change).
    static var isDarkModeOverride: Bool?
}