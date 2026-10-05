/// ProcessesPage.swift — expanded Processes page (PR 2 of dashboard expansion plan).
///
/// Replaces the original Processes subpage layout with an 8-subpage surface
/// driven by `state.processes: [ProcessSummary]` (which now carries
/// `start_time`, `cpu_percent`, `ppid`, `cwd`, `env_count`, `state`,
/// `disk_read_bytes`, `disk_write_bytes`, `fd_count`, and `thread_count`
/// after the sidecar extensions — see `crates/sharecli-ipc/src/handler.rs`).
///
/// Subpages (segmented at top):
///   ┌─────────────────────────────────────────────────────────────────┐
///   │ [All] [By Project] [By Harness]                                 │
///   ├─────────────────────────────────────────────────────────────────┤
///   │ All:     Filter bar + sortable Table (PID/Name/Project/Harness/│
///   │          Memory/Age/Actions) + bulk-action bar + detail drawer  │
///   │ By Proj: Grouped sections w/ project cards (count, RSS, top     │
///   │          harness) + horizontal RSS bar chart + expandable rows  │
///   │ By Har:  Same as By Project but grouped by harness              │
///   └─────────────────────────────────────────────────────────────────┘
///
/// Layout:
///   - Summary strip (4 cards): total / running-aware / total RSS / by project
///   - Filter bar: text + min-RSS slider + bulk-select checkboxes (All only)
///   - Detail drawer (All only): cmd preview + age + per-row kill
///
/// Persistence:
///   - `processes.selectedSubpage` (String: "all" / "byProject" / "byHarness")
///   - `processes.sortFingerprint` (JSON of KeyPathComparator order; restored)
///
/// Bulk actions (All subpage only):
///   - "Kill selected" → state.kill(pid) per selected row
///   - "Kill all" → state.killAll()
///   - "Export JSON" / "Export CSV" → NSSavePanel of filtered set
///
/// Part of: plans/2026-07-25-tray-dashboard-expanded-v1.md §2.1 Page 1.

import SwiftUI
import ShareCLICore
import AppKit
import UniformTypeIdentifiers

// MARK: - Destructive kill confirmation (PLAN.md:236-237, task 1.24)

/// What a destructive kill request covers.
enum KillScope: Equatable {
    case selected(Set<UInt32>)
    case all

    /// Headline for the confirmation dialog.
    var confirmTitle: String {
        switch self {
        case .selected(let pids):
            let n = pids.count
            return "Kill \(n) selected process\(n == 1 ? "" : "es")?"
        case .all:
            return "Kill all processes?"
        }
    }

    /// Body copy: states the consequence plainly.
    var confirmMessage: String {
        switch self {
        case .selected:
            return "Sends SIGTERM to the selected processes. This cannot be undone."
        case .all:
            return "Sends SIGTERM to every process in the fleet pool. This cannot be undone."
        }
    }

    /// Label of the destructive confirm button.
    var confirmButtonLabel: String {
        switch self {
        case .selected: return "Kill"
        case .all: return "Kill all"
        }
    }
}

/// Pure confirmation gate for destructive process actions.
///
/// `request(_:)` records intent without performing it; the caller may only
/// perform the kill once `confirm()` hands the scope back (and it clears
/// pending, so a scope can never fire twice). `decline()` abandons intent.
///
/// Kept free of SwiftUI so "a kill cannot fire without confirmation" is
/// unit-testable without a GUI.
struct DestructiveKillGate: Equatable {
    private(set) var pending: KillScope?

    /// Cancel button label, shared with the confirmation dialogs.
    static let cancelLabel = "Cancel"

    var isConfirming: Bool { pending != nil }

    mutating func request(_ scope: KillScope) { pending = scope }

    mutating func decline() { pending = nil }

    /// Returns the scope to perform and clears it. `nil` means "nothing was
    /// confirmed, perform nothing".
    mutating func confirm() -> KillScope? {
        defer { pending = nil }
        return pending
    }
}

struct ProcessesPage: View {
    @ObservedObject var state: AppState

    @AppStorage("processes.subpage") private var subpageRaw: String = Subpage.all.rawValue
    @State private var subpage: Subpage = .all
    @State private var didLoadSubpage = false

    enum Subpage: String, CaseIterable, Identifiable {
        case all = "all"
        case byProject = "byProject"
        case byHarness = "byHarness"
        case tree = "tree"
        case trends = "trends"
        case resources = "resources"
        case spawn = "spawn"
        case presets = "presets"
        var id: String { rawValue }
        var label: String {
            switch self {
            case .all: return "All"
            case .byProject: return "By Project"
            case .byHarness: return "By Harness"
            case .tree: return "Tree"
            case .trends: return "Trends"
            case .resources: return "Resources"
            case .spawn: return "Spawn"
            case .presets: return "Presets"
            }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: $subpage) {
                ForEach(Subpage.allCases) { sp in
                    Text(sp.label).tag(sp)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .onChange(of: subpage) { _, newValue in
                subpageRaw = newValue.rawValue
            }

            Divider()

            switch subpage {
            case .all: allSubpage
            case .byProject: groupedSubpage(by: \.project, groupLabel: "Project")
            case .byHarness: groupedSubpage(by: \.harness, groupLabel: "Harness")
            case .tree: treeSubpage
            case .trends: trendsSubpage
            case .resources: resourcesSubpage
            case .spawn: spawnSubpage
            case .presets: presetsSubpage
            }
        }
        .frame(minWidth: 720, minHeight: 460)
        .onAppear {
            if !didLoadSubpage {
                subpage = Subpage(rawValue: subpageRaw) ?? .all
                didLoadSubpage = true
            }
        }
    }

    // MARK: - All subpage

    private var allSubpage: some View {
        AllProcessesView(state: state)
    }

    // MARK: - Tree subpage

    private var treeSubpage: some View {
        // P2-9: route to Canvas-based DAG renderer (was TreeView).
        ProcessesTreeCanvasView(state: state, onSelect: { _ in })
    }

    // MARK: - Trends subpage

    private var trendsSubpage: some View {
        // Q8: layer FlameChartView (CPU + Memory + Process count +
        // Network + Load panels) above the existing TrendsView's
        // TrendChartCards via safeAreaInset. Single-line wire; both
        // views coexist.
        TrendsView(state: state).safeAreaInset(edge: .top) { FlameChartView(state: state) }
    }

    // MARK: - Resources subpage

    private var resourcesSubpage: some View {
        ResourcesView(state: state)
    }

    // MARK: - Spawn subpage

    private var spawnSubpage: some View {
        SpawnView(state: state)
    }

    // MARK: - Presets subpage

    private var presetsSubpage: some View {
        PresetsView(state: state)
    }

    // MARK: - Grouped subpage (By Project / By Harness)

    private func groupedSubpage(by keyPath: KeyPath<ProcessSummary, String?>, groupLabel: String) -> some View {
        let groups = groupBy(state.processes, by: keyPath)
        return ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                summaryStripGrouped(groups: groups)
                ForEach(groups, id: \.label) { group in
                    ProjectGroupCard(
                        group: group,
                        keyLabel: groupLabel,
                        kill: { pid in Task { await state.kill(pid: pid) } }
                    )
                }
                if groups.isEmpty {
                    emptyState(message: "No processes to group by \(groupLabel.lowercased()).")
                }
            }
            .padding(16)
        }
    }

    private func summaryStripGrouped(groups: [ProcessGroup]) -> some View {
        let total = state.processes.count
        let totalRSS = state.processes.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
        let projectCount = Set(state.processes.compactMap { $0.project }).count
        let harnessCount = Set(state.processes.compactMap { $0.harness }).count
        return HStack(spacing: 12) {
            summaryCard("Total", "\(total)", "processes", .blue)
            summaryCard("Total RSS", ByteCountFormatter.string(fromByteCount: Int64(totalRSS), countStyle: .memory), "across fleet", .orange)
            summaryCard("Projects", "\(projectCount)", "unique", .purple)
            summaryCard("Harnesses", "\(harnessCount)", "unique", .green)
        }
    }

    private func summaryCard(_ title: String, _ value: String, _ sub: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Empty state

    private func emptyState(message: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "tray")
                .font(.system(size: 36))
                .foregroundStyle(.secondary)
            Text(message)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, minHeight: 240)
        .padding(24)
    }

    // MARK: - Grouping helper

    private func groupBy(_ rows: [ProcessSummary], by keyPath: KeyPath<ProcessSummary, String?>) -> [ProcessGroup] {
        var map: [String: [ProcessSummary]] = [:]
        var order: [String] = []
        for row in rows {
            let key = row[keyPath: keyPath] ?? "(none)"
            if map[key] == nil { order.append(key) }
            map[key, default: []].append(row)
        }
        return order.map { key in
            let members = map[key] ?? []
            let totalRSS = members.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
            // Top harness per project: count of harness values within members
            var harnessCounts: [String: Int] = [:]
            for m in members {
                let h = m.harness ?? "(none)"
                harnessCounts[h, default: 0] += 1
            }
            let topHarness = harnessCounts.max(by: { $0.value < $1.value })?.key ?? "(none)"
            return ProcessGroup(
                label: key,
                members: members.sorted { $0.memory_mb > $1.memory_mb },
                totalRSSBytes: totalRSS,
                topHarness: topHarness,
                uniqueHarnessCount: harnessCounts.count
            )
        }
        .sorted { $0.totalRSSBytes > $1.totalRSSBytes }
    }
}
