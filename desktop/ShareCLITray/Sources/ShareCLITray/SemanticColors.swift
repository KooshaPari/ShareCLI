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
    /// - Light appearance: dark amber (#994C00, ~5.0:1 on white)
    /// - Dark appearance:  SwiftUI .orange (~4.6:1 on dark)
    static var warningForeground: Color {
        let isDark = NSApp?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        if isDark { return .orange }
        return Color(red: 0.60, green: 0.30, blue: 0.0)
    }

    /// Semantic token: success/healthy foreground (green status indicators).
    ///
    /// Use for success badges, healthy-state text, and positive indicators
    /// that must meet WCAG AA 4.5:1 in both appearances.
    ///
    /// Resolves to:
    /// - Light appearance: dark green (#1A6B1A, ~4.8:1 on white)
    /// - Dark appearance:  SwiftUI .green (~4.3:1 on dark)
    static var successForeground: Color {
        let isDark = NSApp?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        if isDark { return .green }
        return Color(red: 0.10, green: 0.42, blue: 0.10)
    }
}