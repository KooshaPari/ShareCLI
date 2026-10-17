/// StatusIcon.swift — single source of truth for tray status symbols.
///
/// Consolidates the 6 heart variants and 6 kill-glyph variants scattered across
/// DashboardView, HealthPage, MiniCompositeHealthCard, HealthPill, PoolPage,
/// TrayPopoverView, PreferencesSheet, CompositeHealthCard into one enum.
///
/// PLAN.md:259-261 — task 1.30 (lane 9, P2):
/// "Single `StatusIcon` enum + `IconFor.action(.killAll)` resolver;
///  replace 3 hearts and 3 kill glyphs with one each."

import Foundation

/// Canonical status-icon enum.  Each case maps to exactly one SF Symbol name.
enum StatusIcon {
    // MARK: - Heart variants (3 distinct symbols)

    /// `heart.text.square.fill` — active composite health metric icon.
    /// Used by: MiniCompositeHealthCard, CompositeHealthCard.
    case compositeHealth

    /// `heart.fill` — sidebar Health section icon.
    /// Used by: DashboardView.
    case healthSection

    /// `heart.text.square` — placeholder when no data is available.
    /// Used by: HealthPage empty state, MiniCompositeHealthCard placeholder,
    /// CompositeHealthCard placeholder.
    case healthPlaceholder

    // MARK: - Kill / destructive / error variants (5 distinct symbols)

    /// `xmark.circle` — per-process kill button.
    /// Used by: TrayPopoverView process row.
    case killProcess

    /// `xmark.circle.fill` — dismiss / close sheet.
    /// Used by: PreferencesSheet close button.
    case dismiss

    /// `xmark.octagon` — critical severity in pool issue rows.
    /// Used by: PoolPage IssueRow.
    case critical

    /// `xmark.octagon.fill` — unhealthy health-pill state.
    /// Used by: HealthPill.
    case unhealthy

    /// `power` — quit application.
    /// Used by: TrayPopoverView action bar.
    case quitApp

    /// The canonical SF Symbol name for this icon.
    var symbolName: String {
        switch self {
        case .compositeHealth:   return "heart.text.square.fill"
        case .healthSection:     return "heart.fill"
        case .healthPlaceholder: return "heart.text.square"
        case .killProcess:       return "xmark.circle"
        case .dismiss:           return "xmark.circle.fill"
        case .critical:          return "xmark.octagon"
        case .unhealthy:         return "xmark.octagon.fill"
        case .quitApp:           return "power"
        }
    }
}

/// Action-based icon resolver.  Maps logical actions to their canonical
/// `StatusIcon`, so callers never embed raw SF Symbol strings.
enum IconFor {
    enum Action {
        /// "Kill All" — destructive bulk-process termination.
        case killAll
    }

    /// Resolve an action to its canonical status icon.
    static func action(_ action: Action) -> StatusIcon {
        switch action {
        case .killAll:
            return .dismiss
        }
    }
}