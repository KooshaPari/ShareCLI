/// CommandPalette.swift — Cmd+K command palette for the dashboard.
///
/// SwiftUI sheet that overlays a fuzzy-search list of all navigable
/// destinations and a curated set of actions (kill all, export, etc).
/// Submits on Enter; Escape closes via .onExitCommand; Up/Down move the
/// highlight; Tab is trapped inside the palette (PLAN.md:220-222).
import SwiftUI
import ShareCLICore

struct CommandPalette: View {
    @ObservedObject var state: AppState
    @Binding var isVisible: Bool
    let onNavigate: (DashboardView.Section) -> Void
    let onAction: (CommandAction) -> Void

    @State private var query: String = ""
    @State private var filtered: [CommandEntry] = []
    @State private var selectedIndex: Int = 0
    @FocusState private var searchFocused: Bool
    /// Destructive action awaiting confirmation (PLAN 1.24). The action is not
    /// forwarded to `onAction` until the operator confirms.
    @State private var pendingDestructive: CommandAction?

    // MARK: - Keyboard contract (PLAN.md:220-222)

    /// What a key press means to the palette. Kept as a pure mapping so the
    /// keyboard behaviour is unit-testable independently of SwiftUI.
    enum KeyIntent: Equatable {
        case moveUp
        case moveDown
        case submit
        case dismiss
        case trapTab
        case passThrough
    }

    /// The palette is a modal surface: nothing behind it may be reached while
    /// it is open, including via assistive technology.
    static let modalAccessibilityTraits: AccessibilityTraits = [.isModal]

    // MARK: - Destructive confirm (PLAN.md:236-237, task 1.24)

    /// Actions that require explicit confirmation before they execute.
    /// Today only `killAll`, which is fleet-wide and irreversible.
    static func isDestructive(_ action: CommandAction) -> Bool {
        action == .killAll
    }

    /// Button role for an action: `.destructive` for kills, `nil` otherwise.
    static func role(for action: CommandAction) -> ButtonRole? {
        isDestructive(action) ? .destructive : nil
    }

    static let destructiveConfirmTitle = "Kill all processes?"
    static let destructiveConfirmMessage =
        "Sends SIGTERM to every process in the fleet pool. This cannot be undone."
    static let destructiveConfirmLabel = "Kill all"
    static let destructiveCancelLabel = "Cancel"

    static func intent(for key: KeyEquivalent) -> KeyIntent {
        switch key {
        case .upArrow: return .moveUp
        case .downArrow: return .moveDown
        case .return: return .submit
        case .escape: return .dismiss
        case .tab: return .trapTab
        default: return .passThrough
        }
    }

    /// Highlight arithmetic: clamped, never wrapping, safe on an empty list.
    static func moveUp(from index: Int) -> Int { max(0, index - 1) }

    static func moveDown(from index: Int, count: Int) -> Int {
        guard count > 0 else { return 0 }
        return min(count - 1, index + 1)
    }

    static func resolvedIndex(_ index: Int, count: Int) -> Int {
        guard count > 0 else { return 0 }
        return min(max(0, index), count - 1)
    }

    enum CommandAction: Hashable {
        case refreshAll
        case killAll
        case exportProcessesJSON
        case exportProcessesCSV
        case clearFilter
        case showHelp
        case openLogFile
        case openPreferences
    }

    private struct CommandEntry: Identifiable, Hashable {
        let id: String
        let title: String
        let subtitle: String
        let icon: String
        enum Kind: Hashable { case navigate(DashboardView.Section), action(CommandAction) }
        let kind: Kind
        static func == (lhs: CommandEntry, rhs: CommandEntry) -> Bool { lhs.id == rhs.id }
        func hash(into hasher: inout Hasher) { hasher.combine(id) }

        /// Destructive role for kill-type actions (PLAN 1.24); nil otherwise.
        var buttonRole: ButtonRole? {
            if case .action(let act) = kind { return CommandPalette.role(for: act) }
            return nil
        }
    }

    private var allEntries: [CommandEntry] {
        let nav: [CommandEntry] = DashboardView.Section.allCases.map { sec in
            CommandEntry(
                id: "nav-\(sec.rawValue)",
                title: sec.rawValue,
                subtitle: sectionSubtitle(sec),
                icon: sec.icon,
                kind: .navigate(sec)
            )
        }
        let acts: [CommandEntry] = [
            .init(id: "act-refresh", title: "Refresh all panels", subtitle: "Force an immediate IPC refresh", icon: "arrow.clockwise", kind: .action(.refreshAll)),
            .init(id: "act-killall", title: "Kill all processes", subtitle: "Send SIGTERM to every process in the fleet pool", icon: "xmark.octagon.fill", kind: .action(.killAll)),
            .init(id: "act-json", title: "Export processes (JSON)", subtitle: "Save the current filtered set to JSON", icon: "square.and.arrow.down", kind: .action(.exportProcessesJSON)),
            .init(id: "act-csv", title: "Export processes (CSV)", subtitle: "Save the current filtered set to CSV", icon: "tablecells", kind: .action(.exportProcessesCSV)),
            .init(id: "act-clearfilter", title: "Clear all filters", subtitle: "Reset text + family + min-RSS", icon: "line.3.horizontal.decrease.circle", kind: .action(.clearFilter)),
            .init(id: "act-help", title: "Show keyboard shortcuts", subtitle: "Reference: ⌘1..8 / ⌘R / ⌘K / ⌘W / ⌘/", icon: "keyboard", kind: .action(.showHelp)),
            .init(id: "act-log", title: "Reveal log file in Finder", subtitle: state.statusSnapshot?.live_log_path?.path ?? "—", icon: "magnifyingglass.circle", kind: .action(.openLogFile)),
            .init(id: "act-prefs", title: "Open preferences", subtitle: "Refresh interval · log buffer cap · IPC endpoint", icon: "gearshape", kind: .action(.openPreferences)),
        ]
        return nav + acts
    }

    private func sectionSubtitle(_ sec: DashboardView.Section) -> String {
        switch sec {
        case .overview: return "Aggregated fleet + host + activity grid"
        case .processes: return "All live processes with bulk actions"
        case .agents: return "Spawned fleet agents"
        case .pool: return "Runtime pool composition + gate"
        case .effectiveness: return "Coalesce cache + slot-queue meters"
        case .config: return "Live config editor + JSON preview"
        case .health: return "Memory / thermal / host resource watch"
        case .logs: return "Live log tail with filter + export"
        }
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.42)
                .ignoresSafeArea()
                .onTapGesture { isVisible = false }
            VStack(spacing: 0) {
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(.secondary)
                    TextField("Search pages + actions…", text: $query)
                        .textFieldStyle(.plain)
                        .font(.title3)
                        .focused($searchFocused)
                        .filterFieldFocus($searchFocused)
                        .onSubmit { submitHighlighted() }
                    if !query.isEmpty {
                        Button {
                            query = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                        }
                        .buttonStyle(.pressable)
                    }
                    Text("esc")
                        .font(.caption2.monospaced())
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.quaternary)
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }
                .padding(14)
                Divider()
                if filtered.isEmpty {
                    VStack(spacing: 8) {
                        Image(systemName: "magnifyingglass")
                            .font(.system(size: 32))
                            .foregroundStyle(.tertiary)
                        Text("No matches for \"\(query)\"")
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, minHeight: 200)
                } else {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(filtered.enumerated()), id: \.element.id) { i, entry in
                                Button(role: entry.buttonRole) {
                                    submit(entry)
                                } label: {
                                    HStack(spacing: 12) {
                                        Image(systemName: entry.icon)
                                            .frame(width: 22)
                                            .foregroundStyle(i == selectedIndex ? Color.white : Color.secondary)
                                        VStack(alignment: .leading, spacing: 1) {
                                            Text(entry.title)
                                                .font(.body)
                                                .foregroundStyle(i == selectedIndex ? Color.white : Color.primary)
                                            Text(entry.subtitle)
                                                .font(.caption2)
                                                .foregroundStyle(i == selectedIndex ? Color.white.opacity(0.7) : Color.secondary)
                                                .lineLimit(1)
                                        }
                                        Spacer()
                                        if i == selectedIndex {
                                            Text("↵")
                                                .font(.caption.monospaced())
                                                .foregroundStyle(.white.opacity(0.6))
                                        }
                                    }
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 8)
                                    .background(i == selectedIndex ? Color.accentColor : Color.clear)
                                    .clipShape(RoundedRectangle(cornerRadius: 4))
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.pressable)
                            }
                        }
                        .padding(8)
                    }
                }
            }
            .frame(width: 560, height: 380)
            .background(.regularMaterial)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .shadow(color: .black.opacity(0.30), radius: 24, y: 8)
            .onExitCommand { isVisible = false }
            .onKeyPress(.upArrow) { handle(Self.intent(for: .upArrow)) }
            .onKeyPress(.downArrow) { handle(Self.intent(for: .downArrow)) }
            .onKeyPress(.tab) { handle(Self.intent(for: .tab)) }
            .accessibilityElement(children: .contain)
            .accessibilityAddTraits(Self.modalAccessibilityTraits)
        }
        .transition(.scale(scale: 0.96).combined(with: .opacity))
        .onAppear {
            filtered = allEntries
            selectedIndex = 0
            searchFocused = true
        }
        .onChange(of: query) { _, newValue in
            let q = newValue.lowercased().trimmingCharacters(in: .whitespaces)
            if q.isEmpty {
                filtered = allEntries
            } else {
                filtered = allEntries.filter { e in
                    e.title.lowercased().contains(q)
                        || e.subtitle.lowercased().contains(q)
                        || e.icon.contains(q)
                }
            }
            selectedIndex = 0
        }
        .confirmationDialog(
            Self.destructiveConfirmTitle,
            isPresented: Binding(
                get: { pendingDestructive != nil },
                set: { if !$0 { pendingDestructive = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button(Self.destructiveConfirmLabel, role: .destructive) {
                confirmDestructive()
            }
            Button(Self.destructiveCancelLabel, role: .cancel) {
                pendingDestructive = nil
            }
        } message: {
            Text(Self.destructiveConfirmMessage)
        }
    }

    /// Routes a key intent to the palette. Only the keys named by
    /// `intent(for:)` are intercepted; everything else falls through to the
    /// focused text field.
    private func handle(_ intent: KeyIntent) -> KeyPress.Result {
        switch intent {
        case .moveUp:
            selectedIndex = Self.moveUp(from: selectedIndex)
        case .moveDown:
            selectedIndex = Self.moveDown(from: selectedIndex, count: filtered.count)
        case .submit:
            submitHighlighted()
        case .dismiss:
            isVisible = false
        case .trapTab:
            searchFocused = true
        case .passThrough:
            return .ignored
        }
        return .handled
    }

    private func submitHighlighted() {
        let idx = Self.resolvedIndex(selectedIndex, count: filtered.count)
        guard filtered.indices.contains(idx) else { return }
        submit(filtered[idx])
    }

    private func submit(_ entry: CommandEntry) {
        switch entry.kind {
        case .navigate(let sec):
            onNavigate(sec)
            isVisible = false
        case .action(let act):
            if Self.isDestructive(act) {
                // Do not forward yet: the confirmation dialog gates execution,
                // and the palette stays open until the operator decides.
                pendingDestructive = act
                return
            }
            onAction(act)
            isVisible = false
        }
    }

    /// Runs the confirmed destructive action and closes the palette.
    private func confirmDestructive() {
        guard let act = pendingDestructive else { return }
        pendingDestructive = nil
        isVisible = false
        onAction(act)
    }
}
