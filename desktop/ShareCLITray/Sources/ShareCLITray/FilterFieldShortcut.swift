import SwiftUI

/// PLAN.md:239-240 — task 1.25 (lane 9, P1):
/// "Tray: ⌘F filter focus (lane 9, P1)
///  - Four filter fields get a `.keyboardShortcut("f", modifiers: .command)`."
///
/// FR: FR-007 (tray/desktop consumer parity, AC-007.47–AC-007.54). PLAN.md names
/// no FR/AC id for task 1.25; FR-007 governs the tray surface as a whole and is
/// the nearest honest anchor. See `FilterFieldShortcutTests` for the asserted
/// contract.
///
/// The app has exactly four filter / search text fields:
///   • Processes → All subpage filter (`ProcessesPage.swift`)
///   • Agents filter (`AgentsPage.swift`)
///   • Logs filter (`LogsPage.swift`)
///   • ⌘K palette search (`CommandPalette.swift`)
/// The first three are wired here. The palette's search field is the fourth but
/// is owned by lane 1.20 (`CommandPalette.swift` is explicitly out of scope for
/// task 1.25), so it is recorded in `outOfScopeFields` rather than silently
/// dropped or falsely claimed.
enum FilterFieldShortcut {

    /// The key and modifiers the plan specifies: `.keyboardShortcut("f",
    /// modifiers: .command)`.
    static let key: KeyEquivalent = "f"
    static let modifiers: EventModifiers = .command

    /// Identity of each filter field that carries the shortcut.
    enum Field: String, CaseIterable, Identifiable {
        case processesFilter = "processes.filter"
        case agentsFilter = "agents.filter"
        case logsFilter = "logs.filter"
        /// The ⌘K palette's search field — fourth filter field in the app,
        /// owned by lane 1.20 (task 1.20, CommandPalette.swift).
        case paletteSearch = "palette.search"

        var id: String { rawValue }
    }

    /// Fields wired by task 1.25 plus the closure batch (⌘F palette search).
    static let wiredFields: [Field] = [.processesFilter, .agentsFilter, .logsFilter, .paletteSearch]

    /// Fields deliberately deferred to another lane (recorded, not claimed).
    ///
    /// Closure batch (cricket 2026-10-05): `paletteSearch` was the sole deferred
    /// field and is now wired, so this list is empty. Kept so a future deferral
    /// has a home that the tests already check.
    static let outOfScopeFields: [Field] = []

    /// App-global shortcuts owned by `DashboardView.attachShortcutMonitor`
    /// (⌘1..⌘8 page jumps, ⌘K palette, ⌘R refresh, ⌘W close, ⌘/ help, ⌘, prefs).
    /// Kept here so a future ⌘F wiring cannot be validated against a stale list.
    static let reservedGlobalShortcuts: [KeyEquivalent] = [
        "1", "2", "3", "4", "5", "6", "7", "8",
        "k", "r", "w", "/", ",",
    ]

    /// True when `key` is already claimed by a dashboard-level shortcut.
    static func collidesWithReserved(_ key: KeyEquivalent) -> Bool {
        reservedGlobalShortcuts.contains(key)
    }
}

extension View {
    /// Attaches ⌘F to a filter field and moves keyboard focus into it.
    ///
    /// `.keyboardShortcut(_:modifiers:)` is inert on a `TextField`: it assigns a
    /// shortcut to the view's *action*, and a text field has no action, so
    /// nothing receives the key. The shortcut is therefore carried by a
    /// zero-size, accessibility-hidden button that flips the field's
    /// `@FocusState`. The key and modifiers still come from
    /// `FilterFieldShortcut`, i.e. exactly `"f" + .command` per the plan.
    func filterFieldFocus(_ isFocused: FocusState<Bool>.Binding) -> some View {
        modifier(FilterFieldFocusModifier(isFocused: isFocused))
    }
}

private struct FilterFieldFocusModifier: ViewModifier {
    @FocusState.Binding var isFocused: Bool

    func body(content: Content) -> some View {
        content
            .focused($isFocused)
            .background(alignment: .topLeading) {
                Button("Focus filter") { isFocused = true }
                    .keyboardShortcut(
                        FilterFieldShortcut.key,
                        modifiers: FilterFieldShortcut.modifiers
                    )
                    .frame(width: 0, height: 0)
                    .opacity(0)
                    .accessibilityHidden(true)
            }
    }
}
