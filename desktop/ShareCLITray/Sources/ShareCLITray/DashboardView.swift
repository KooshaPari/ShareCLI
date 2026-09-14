/// DashboardView.swift — full NSWindow dashboard with 8 sidebar pages.
///
/// Navigation sidebar (Cmd+1..8 to jump):
///   1. Overview     → aggregated fleet + host + activity dashboard grid (q-dash-2)
///   2. Processes    → live process table with filter + bulk kill + export (PR 2)
///   3. Agents       → fleet agent process roster (PR 1)
///   4. Pool         → runtime pool composition + issues + host watch (PR 3)
///   5. Effectiveness → cache/slot-queue hit rate counters from sharecli-fleet (PR 4)
///   6. Config       → spawn_policy + pool + monitoring config editor (PR 7)
///   7. Health       → Memory / Thermal gate / Host watch + subpages (PR 6)
///   8. Logs         → live-tailing log viewer with filter + export (PR 8)

import SwiftUI
import ShareCLICore

struct DashboardView: View {
    @ObservedObject var state: AppState
    @AppStorage("dashboard.sidebar.selection") private var selectionRaw: String = Section.overview.rawValue
    @AppStorage("dashboard.sidebar.columnWidth") private var sidebarColumnWidth: Double = 200
    @State private var paletteVisible: Bool = false
    @State private var helpVisible: Bool = false
    @State private var prefsVisible: Bool = false
    @State private var updaterVisible: Bool = false
    @AppStorage(UpdateChannel.storageKey) private var channelRaw: String = UpdateChannel.default.rawValue
    @State private var windowWidth: CGFloat = 900
    @State private var hoveredSection: Section?
    private var channel: UpdateChannel { UpdateChannel(rawValue: channelRaw) ?? .default }

    private var selection: Binding<Section> {
        Binding(
            get: { Section(rawValue: selectionRaw) ?? .overview },
            set: { selectionRaw = $0.rawValue }
        )
    }

    /// Sidebar is icon-only when window is narrow (< 800 pt).
    private var isCompactSidebar: Bool { windowWidth < 800 }

    enum Section: String, CaseIterable, Identifiable {
        case overview = "Overview"
        case processes = "Processes"
        case agents = "Agents"
        case pool = "Pool"
        case effectiveness = "Pool effectiveness"
        case config = "Config"
        case health = "Health"
        case logs = "Logs"
        var id: String { rawValue }
        var icon: String {
            switch self {
            case .overview: return "rectangle.grid.2x2"
            case .processes: return "cpu"
            case .agents: return "person.2.fill"
            case .pool: return "rectangle.stack.fill"
            case .effectiveness: return "chart.line.uptrend.xyaxis"
            case .config: return "gearshape"
            case .health: return "heart.fill"
            case .logs: return "text.alignleft"
            }
        }
        /// Cmd+N index for keyboard shortcuts (1-based per the plan §5.4).
        var shortcutIndex: Int {
            Section.allCases.firstIndex(of: self).map { $0 + 1 } ?? 0
        }
    }

    var body: some View {
        NavigationSplitView {
            sidebar
        } detail: {
            detail
                .frame(minWidth: 600)
        }
        .navigationSplitViewColumnWidth(
            min: isCompactSidebar ? 48 : 160,
            ideal: isCompactSidebar ? 48 : sidebarColumnWidth
        )
        .frame(minWidth: 900, minHeight: 560)
        .background(
            GeometryReader { geo in
                Color.clear.onAppear { windowWidth = geo.size.width }
                    .onChange(of: geo.size.width) { _, newWidth in windowWidth = newWidth }
            }
        )
        .background(WindowAccessor { window in
            attachShortcutMonitor(window: window)
        })
        .toolbar { toolbar }
        .overlay {
            if paletteVisible {
                CommandPalette(
                    state: state,
                    isVisible: $paletteVisible,
                    onNavigate: { sec in selectionRaw = sec.rawValue },
                    onAction: { action in handleAction(action) }
                )
            }
        }
        .sheet(isPresented: $helpVisible) {
            HelpSheet(isVisible: $helpVisible)
        }
        .sheet(isPresented: $prefsVisible) {
            PreferencesSheet(isVisible: $prefsVisible, state: state)
        }
        .onAppear {
            // Persist a sensible default if the @AppStorage above was missing.
            if Section(rawValue: selectionRaw) == nil {
                selectionRaw = Section.overview.rawValue
            }
        }
    }

    @ViewBuilder
    private var sidebar: some View {
        List(Section.allCases, selection: selection) { sec in
            sidebarRow(sec)
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            MiniCompositeHealthCard(
                fleet: state.fleetHistory.last,
                host: state.hostWatchHistory.last
            )
            .padding(.horizontal, 8)
            .padding(.bottom, 8)
        }
    }

    @ViewBuilder
    private func sidebarRow(_ sec: Section) -> some View {
        let isSelected = selection.wrappedValue == sec
        let isHovered = hoveredSection == sec

        HStack(spacing: 8) {
            Image(systemName: sec.icon)
                .font(.system(size: 14))
                .frame(width: 18)
            if !isCompactSidebar {
                Text(sec.rawValue)
                    .font(.system(size: 13))
                    .lineLimit(1)
                Spacer()
                if sec.shortcutIndex > 0 {
                    Text("⌘\(sec.shortcutIndex)")
                        .font(.caption2.monospaced())
                        .foregroundStyle(.tertiary)
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(isSelected ? Color.accentColor.opacity(0.15)
                      : isHovered ? Color.primary.opacity(0.06)
                      : Color.clear)
        )
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredSection = hovering ? sec : nil
        }
        .onTapGesture {
            selection.wrappedValue = sec
        }
        .help(isCompactSidebar ? "\(sec.rawValue) (⌘\(sec.shortcutIndex))" : sectionSummary(sec))
    }

    @ViewBuilder
    private var detail: some View {
        switch Section(rawValue: selectionRaw) ?? .overview {
        case .overview: DashboardOverview(state: state)
        case .processes: ProcessesPage(state: state)
        case .agents: AgentsPage(state: state)
        case .pool: PoolPage(state: state)
        case .effectiveness: PoolEffectivenessPage(state: state)
        case .config: ConfigPage(state: state)
        case .health: HealthPage(state: state)
        case .logs: LogsPage(state: state)
        }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .primaryAction) {
            Button {
                prefsVisible = true
            } label: {
                Label("Preferences", systemImage: "gearshape")
            }
            .help("Open preferences (⌘,)")
        }
        ToolbarItem(placement: .primaryAction) {
            Button {
                Task { await state.refresh() }
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .help("Refresh all panels (⌘R)")
        }
        ToolbarItem(placement: .primaryAction) {
            if let err = state.lastError {
                Label(err, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                    .help(err)
            }
        }
        ToolbarItem(placement: .primaryAction) {
            Menu {
                ForEach(UpdateChannel.allCases) { option in
                    Button {
                        channelRaw = option.rawValue
                    } label: {
                        if channelRaw == option.rawValue {
                            Label(option.displayName, systemImage: "checkmark")
                        } else {
                            Text(option.displayName)
                        }
                    }
                }
                Divider()
                Button("Check for updates…") { updaterVisible = true }
            } label: {
                Label("Channel: \(channel.displayName)", systemImage: "sparkles")
                    .foregroundStyle(channel.badgeColor)
            }
            .help("Sparkle release channel (current: \(channel.displayName))")
        }
        ToolbarItem(placement: .primaryAction) { Button { updaterVisible.toggle() } label: { Label("Updates", systemImage: "sparkles") }.help("Check for updates (Sparkle)").popover(isPresented: $updaterVisible, arrowEdge: .bottom) { UpdaterView(feedURL: URL(string: "https://sharecli.example/appcast.xml")!, publicEdKey: nil).frame(width: 360) } }
    }

    // MARK: - Keyboard shortcut plumbing

    private func attachShortcutMonitor(window: NSWindow?) -> Void {
        // Use a local event monitor so Cmd+1..8 work even when focus is on
        // a SwiftUI subview (SwiftUI's .keyboardShortcut(_, modifiers:)
        // attaches to a specific button; we want it for the whole window).
        let monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.modifierFlags.contains(.command),
               let chars = event.charactersIgnoringModifiers,
               let c = chars.first,
               let n = Int(String(c)),
               (1...Section.allCases.count).contains(n),
               let sec = Section.allCases[safe: n - 1] {
                self.selectionRaw = sec.rawValue
                NSLog("[DashboardView] Cmd+%d → %@", n, sec.rawValue)
                return nil
            }
            if event.modifierFlags.contains(.command),
               event.charactersIgnoringModifiers == "r" {
                Task { await state.refresh() }
                return nil
            }
            if event.modifierFlags.contains(.command),
               event.charactersIgnoringModifiers == "k" {
                self.paletteVisible.toggle()
                return nil
            }
            if event.modifierFlags.contains(.command),
               event.charactersIgnoringModifiers == "w" {
                NSApp.keyWindow?.performClose(nil)
                return nil
            }
            if event.modifierFlags.contains(.command),
               event.charactersIgnoringModifiers == "/" {
                self.helpVisible.toggle()
                return nil
            }
            if event.modifierFlags.contains(.command),
               event.charactersIgnoringModifiers == "," {
                self.prefsVisible.toggle()
                return nil
            }
            return event
        }
        if let window = window, let token = monitor {
            // Keep the monitor alive at least as long as the window.
            objc_setAssociatedObject(window, &Self.monitorKey, token, .OBJC_ASSOCIATION_RETAIN)
        }
    }

    private func handleAction(_ action: CommandPalette.CommandAction) {
        switch action {
        case .refreshAll:
            Task { await state.refresh() }
        case .killAll:
            Task { await state.killAll() }
        case .exportProcessesJSON, .exportProcessesCSV, .clearFilter:
            // Navigate to Processes page; the user can use the toolbar there.
            selectionRaw = Section.processes.rawValue
        case .showHelp:
            helpVisible = true
        case .openPreferences:
            prefsVisible = true
        case .openLogFile:
            if let url = state.statusSnapshot?.live_log_path {
                NSWorkspace.shared.activateFileViewerSelecting([url])
            }
        }
    }

    private static var monitorKey: UInt8 = 0

    // MARK: - Helpers

    private func sectionSummary(_ sec: Section) -> String {
        switch sec {
        case .overview: return "Aggregated fleet + host + activity dashboard grid. ⌘1"
        case .processes: return "All live processes — bulk kill, JSON/CSV export, by-project/by-harness grouping. ⌘2"
        case .agents: return "Spawned fleet agents (claude / forge / node / bun / etc). ⌘3"
        case .pool: return "Runtime pool composition + gate decisions + host watch sparklines. ⌘4"
        case .effectiveness: return "Hypervisor coalesce cache + slot-queue counters. ⌘5"
        case .config: return "Spawn policy / pool / monitoring config editor with live apply. ⌘6"
        case .health: return "Memory / Thermal gate / Host resource watch with subpages. ⌘7"
        case .logs: return "Live tail of ~/.sharecli/logs/sharecli.log with filter + export. ⌘8"
        }
    }
}

private extension Array {
    subscript(safe idx: Int) -> Element? {
        indices.contains(idx) ? self[idx] : nil
    }
}

// MARK: - Window accessor (SwiftUI 4+ on macOS)

private struct WindowAccessor: NSViewRepresentable {
    let onResolve: (NSWindow?) -> Void
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async { [weak view] in
            onResolve(view?.window)
        }
        return view
    }
    func updateNSView(_ nsView: NSView, context: Context) {}
}
