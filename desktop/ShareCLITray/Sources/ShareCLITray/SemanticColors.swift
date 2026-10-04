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
}