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

// MARK: - ProcessGroup data

struct ProcessGroup: Hashable {
    let label: String
    let members: [ProcessSummary]
    let totalRSSBytes: UInt64
    let topHarness: String
    let uniqueHarnessCount: Int
}

// MARK: - ProjectGroupCard (renders one grouped section)

struct ProjectGroupCard: View {
    let group: ProcessGroup
    let keyLabel: String
    let kill: (UInt32) -> Void
    @State private var expanded = true

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(keyLabel).font(.caption2).foregroundStyle(.secondary)
                        Text(group.label)
                            .font(.system(.headline, design: .monospaced))
                            .bold()
                    }
                    HStack(spacing: 12) {
                        Label("\(group.members.count)", systemImage: "cpu")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Label(
                            ByteCountFormatter.string(fromByteCount: Int64(group.totalRSSBytes), countStyle: .memory),
                            systemImage: "memorychip"
                        )
                        .font(.caption)
                        .foregroundStyle(.orange)
                        Label("top: \(group.topHarness)", systemImage: "tag")
                            .font(.caption)
                            .foregroundStyle(.purple)
                        if group.uniqueHarnessCount > 1 {
                            Text("+\(group.uniqueHarnessCount - 1) more harness")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Spacer()
                Button {
                    withAnimation(.easeInOut(duration: 0.15)) { expanded.toggle() }
                } label: {
                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                }
                .buttonStyle(.borderless)
            }

            // RSS bar — relative to max group in this set
            if expanded {
                Divider()
                rssBarChart
                VStack(spacing: 4) {
                    ForEach(group.members.prefix(5), id: \.pid) { p in
                        ProcessRowInline(p: p, kill: kill)
                    }
                    if group.members.count > 5 {
                        Text("+\(group.members.count - 5) more (sorted by RSS desc)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
        }
        .padding(12)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var rssBarChart: some View {
        let maxBytes = group.members.map { $0.memory_mb }.max() ?? 1
        return VStack(spacing: 2) {
            ForEach(group.members.prefix(8), id: \.pid) { p in
                let frac = Double(p.memory_mb) / Double(max(maxBytes, 1))
                HStack(spacing: 6) {
                    Text(p.name)
                        .font(.system(.caption, design: .monospaced))
                        .frame(width: 110, alignment: .leading)
                        .lineLimit(1)
                    GeometryReader { geo in
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 3).fill(.quaternary.opacity(0.5))
                            RoundedRectangle(cornerRadius: 3)
                                .fill(rssBarColor(p.memory_mb))
                                .frame(width: max(2, geo.size.width * CGFloat(frac)))
                        }
                    }
                    .frame(height: 10)
                    Text("\(p.memory_mb) MB")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .frame(width: 60, alignment: .trailing)
                }
            }
        }
    }

    private func rssBarColor(_ mb: UInt64) -> Color {
        if mb > 1024 { return .red }
        if mb > 512 { return .orange }
        if mb > 128 { return .yellow }
        return .blue
    }
}

struct ProcessRowInline: View {
    let p: ProcessSummary
    let kill: (UInt32) -> Void
    var body: some View {
        HStack(spacing: 8) {
            Text("\(p.pid)").font(.system(.caption, design: .monospaced)).frame(width: 50, alignment: .leading)
            Text(p.name).font(.system(.caption, design: .monospaced)).frame(width: 140, alignment: .leading).lineLimit(1)
            if let proj = p.project { Badge(text: proj, color: .blue) }
            if let h = p.harness { Badge(text: h, color: .purple) }
            Spacer()
            Text("\(p.memory_mb) MB")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)
            Button {
                kill(p.pid)
            } label: {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
            }
            .buttonStyle(.borderless)
            .help("Kill PID \(p.pid)")
        }
    }
}

// MARK: - AllProcessesView

struct AllProcessesView: View {
    @ObservedObject var state: AppState

    @State private var filterText: String = ""
    @State private var minRSS: Double = 0  // MB threshold
    @State private var sortOrder: [KeyPathComparator<ProcessSummary>] = [
        KeyPathComparator(\ProcessSummary.memory_mb, order: .reverse)
    ]
    @State private var selection: Set<UInt32> = []
    @State private var bulkStatus: String = ""

    private var filtered: [ProcessSummary] {
        let q = filterText.lowercased()
        let minBytes = UInt64(max(0, minRSS)) * 1024 * 1024
        return state.processes.filter { p in
            if minBytes > 0 && p.memory_mb * 1024 * 1024 < minBytes { return false }
            if q.isEmpty { return true }
            return p.name.lowercased().contains(q)
                || (p.project?.lowercased().contains(q) ?? false)
                || (p.harness?.lowercased().contains(q) ?? false)
                || String(p.pid).contains(q)
        }
        .sorted(using: sortOrder)
    }

    var body: some View {
        VStack(spacing: 0) {
            summaryStrip
            filterBar
            bulkActionBar
            Divider()
            if filtered.isEmpty {
                EmptyStateView(
                    icon: "tray",
                    title: state.processes.isEmpty
                        ? "No processes yet"
                        : "No processes match your filter",
                    subtitle: state.processes.isEmpty
                        ? "The fleet pool will list registered processes here once the host directory contains bun sockets or any host-tracked CLI processes."
                        : "Try widening the text filter, lowering the minimum RSS slider, or clearing the filter entirely.",
                    variant: .hero,
                    primaryTitle: state.processes.isEmpty ? "Refresh now" : "Clear filter",
                    primaryIcon: state.processes.isEmpty ? "arrow.clockwise" : "xmark.circle",
                    primaryAction: {
                        if state.processes.isEmpty {
                            Task { await state.refresh() }
                        } else {
                            filterText = ""
                            minRSS = 0
                        }
                    },
                    secondaryTitle: state.processes.isEmpty ? "How does this work?" : nil,
                    secondaryIcon: "questionmark.circle",
                    secondaryAction: state.processes.isEmpty ? {
                        if let url = URL(string: "https://docs.sharecli.dev/processes") { NSWorkspace.shared.open(url) }
                    } : nil
                )
            } else {
                Table(filtered, selection: $selection, sortOrder: $sortOrder) {
                TableColumn("PID", value: \.pid) { p in
                    Text("\(p.pid)").font(.system(.body, design: .monospaced))
                }
                .width(60)

                TableColumn("Name", value: \.name) { p in
                    Text(p.name).font(.system(.body, design: .monospaced)).lineLimit(1)
                }

                TableColumn("Project") { p in
                    if let proj = p.project { Badge(text: proj, color: .blue) }
                }
                .width(100)

                TableColumn("Harness") { p in
                    if let h = p.harness { Badge(text: h, color: .purple) }
                }
                .width(80)

                TableColumn("Memory (MB)", value: \.memory_mb) { p in
                    Text("\(p.memory_mb)").font(.system(.body, design: .monospaced))
                        .foregroundStyle(rssColor(p.memory_mb))
                }
                .width(110)

                TableColumn("CPU %", value: \.cpu_percent) { p in
                    Text(String(format: "%.1f%%", p.cpu_percent))
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(cpuColor(p.cpu_percent))
                        .frame(width: 56, alignment: .trailing)
                    cpuBar(p.cpu_percent)
                }
                .width(140)

                TableColumn("FDs", value: \.fdCountValue) { p in
                    if let fd = p.fd_count {
                        HStack(spacing: 4) {
                            Text("\(fd)")
                                .font(.system(.body, design: .monospaced))
                                .foregroundStyle(fdColor(fd))
                                .frame(width: 38, alignment: .trailing)
                            fdBar(fd)
                        }
                    } else {
                        Text("n/a")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.tertiary)
                    }
                }
                .width(96)

                TableColumn("I/O", value: \.ioReadValue) { p in
                    if let r = p.disk_read_bytes, let w = p.disk_write_bytes {
                        VStack(alignment: .trailing, spacing: 1) {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.down.circle.fill")
                                    .font(.caption2)
                                    .foregroundStyle(.blue)
                                Text(ioBytes(r))
                                    .font(.system(.caption, design: .monospaced))
                            }
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.up.circle.fill")
                                    .font(.caption2)
                                    .foregroundStyle(.purple)
                                Text(ioBytes(w))
                                    .font(.system(.caption, design: .monospaced))
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .trailing)
                    } else {
                        Text("n/a")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.tertiary)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                    }
                }
                .width(120)

                TableColumn("Age") { p in
                    Text(formatAge(p.start_time))
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                .width(80)

                TableColumn("Actions") { p in
                    Button {
                        Task { await state.kill(pid: p.pid) }
                    } label: {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
                    }
                    .buttonStyle(.borderless)
                    .help("Kill PID \(p.pid) (\(p.name))")
                }
                .width(40)
            }
            .frame(minHeight: 240)
            } // end Table
        }
    }

    // MARK: - Summary

    private var summaryStrip: some View {
        let total = state.processes.count
        let filteredCount = filtered.count
        let totalRSSBytes = state.processes.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
        let selectedRSS = filtered
            .filter { selection.contains($0.pid) }
            .reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
        let topCPU = state.processes
            .max(by: { $0.cpu_percent < $1.cpu_percent })?.cpu_percent ?? 0
        return HStack(spacing: 12) {
            summaryCard("Total", "\(total)", filteredCount == total ? "processes" : "\(filteredCount) shown", .blue)
            summaryCard("Total RSS", ByteCountFormatter.string(fromByteCount: Int64(totalRSSBytes), countStyle: .memory), "fleet", .orange)
            summaryCard("Selected", "\(selection.count)", selection.isEmpty ? "—" : ByteCountFormatter.string(fromByteCount: Int64(selectedRSS), countStyle: .memory), .purple)
            summaryCard("Top CPU", String(format: "%.1f%%", topCPU), "fleet peak", cpuColor(topCPU))
            summaryCard("Filtered", "\(filteredCount)", "of \(total)", .green)
        }
        .padding(10)
        .background(.quaternary.opacity(0.5))
    }

    private func summaryCard(_ title: String, _ value: String, _ sub: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    // MARK: - Filter bar

    private var filterBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Filter by name / project / harness / pid", text: $filterText)
                .textFieldStyle(.plain)
            if !filterText.isEmpty {
                Button { filterText = "" } label: {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
            }
            Divider().frame(height: 16)
            Text("Min RSS \(Int(minRSS)) MB").font(.caption).foregroundStyle(.secondary)
            Slider(value: $minRSS, in: 0...4096, step: 64) {
                Text("Min RSS")
            }
            .frame(width: 160)
            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.3))
    }

    // MARK: - Bulk action bar

    private var bulkActionBar: some View {
        HStack(spacing: 10) {
            Button {
                Task {
                    var killed = 0
                    for pid in selection {
                        await state.kill(pid: pid)
                        killed += 1
                    }
                    bulkStatus = "Killed \(killed) selected"
                    try? await Task.sleep(nanoseconds: 2_000_000_000)
                    bulkStatus = ""
                }
            } label: {
                Label("Kill selected (\(selection.count))", systemImage: "xmark.circle")
            }
            .disabled(selection.isEmpty)

            Button {
                Task {
                    await state.killAll()
                    bulkStatus = "Kill-all requested"
                    try? await Task.sleep(nanoseconds: 2_000_000_000)
                    bulkStatus = ""
                }
            } label: {
                Label("Kill all", systemImage: "xmark.octagon")
            }
            .tint(.red)

            Spacer()

            Button {
                exportJSON(filtered: filtered)
            } label: {
                Label("Export JSON", systemImage: "square.and.arrow.up")
            }

            Button {
                exportCSV(filtered: filtered)
            } label: {
                Label("Export CSV", systemImage: "tablecells")
            }

            // P2-11: export only the rows currently selected in the Table
            Button {
                exportSelectedJSON()
            } label: {
                Label("Export selected JSON (\(selection.count))", systemImage: "square.and.arrow.down.on.square")
            }
            .disabled(selection.isEmpty)

            Button {
                exportSelectedCSV()
            } label: {
                Label("Export selected CSV (\(selection.count))", systemImage: "tablecells.badge.ellipsis")
            }
            .disabled(selection.isEmpty)

            if !bulkStatus.isEmpty {
                Text(bulkStatus)
                    .font(.caption)
                    .foregroundStyle(bulkStatus.lowercased().contains("error") ? .red : .green)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.3))
    }

    // MARK: - Export

    private func exportJSON(filtered: [ProcessSummary]) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType.json]
        panel.nameFieldStringValue = "sharecli-processes.json"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }

        struct Out: Codable { let exported_at: UInt64; let count: Int; let processes: [ProcessSummary] }
        let payload = Out(
            exported_at: UInt64(Date().timeIntervalSince1970),
            count: filtered.count,
            processes: filtered
        )
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(payload) {
            try? data.write(to: url)
        }
    }

    private func exportCSV(filtered: [ProcessSummary]) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType.commaSeparatedText]
        panel.nameFieldStringValue = "sharecli-processes.csv"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }

        var lines: [String] = ["pid,name,memory_mb,project,harness,start_time,age_seconds,cpu_percent"]
        let now = UInt64(Date().timeIntervalSince1970)
        for p in filtered {
            let age = p.start_time > 0 ? (now >= p.start_time ? now - p.start_time : 0) : 0
            let cells: [String] = [
                "\(p.pid)",
                csvEscape(p.name),
                "\(p.memory_mb)",
                csvEscape(p.project ?? ""),
                csvEscape(p.harness ?? ""),
                "\(p.start_time)",
                "\(age)",
                String(format: "%.2f", p.cpu_percent)
            ]
            lines.append(cells.joined(separator: ","))
        }
        let body = lines.joined(separator: "\n") + "\n"
        try? body.write(to: url, atomically: true, encoding: .utf8)
    }

    private func exportSelectedJSON() {
        let sel = selectedRows()
        if sel.isEmpty { return }
        exportJSON(filtered: sel)
    }

    private func exportSelectedCSV() {
        let sel = selectedRows()
        if sel.isEmpty { return }
        exportCSV(filtered: sel)
    }

    private func selectedRows() -> [ProcessSummary] {
        // `selection` is Set<ProcessSummary.ID> (UInt32 pid); map back to rows.
        let pidSet = selection
        return filtered.filter { pidSet.contains($0.pid) }
    }

    private func csvEscape(_ s: String) -> String {
        if s.contains(",") || s.contains("\"") || s.contains("\n") {
            return "\"" + s.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        return s
    }

    // MARK: - Helpers

    private func formatAge(_ startTime: UInt64) -> String {
        guard startTime > 0 else { return "—" }
        let now = UInt64(Date().timeIntervalSince1970)
        guard now >= startTime else { return "?" }
        let secs = now - startTime
        if secs < 60 { return "\(secs)s" }
        if secs < 3600 { return "\(secs / 60)m" }
        if secs < 86400 { return "\(secs / 3600)h \(secs % 3600 / 60)m" }
        return "\(secs / 86400)d"
    }

    private func rssColor(_ mb: UInt64) -> Color {
        if mb > 1024 { return .red }
        if mb > 512 { return .orange }
        if mb > 128 { return .yellow }
        return .primary
    }

    private func cpuColor(_ pct: Float) -> Color {
        if pct > 90 { return .red }
        if pct > 60 { return .orange }
        if pct > 25 { return .yellow }
        return .secondary
    }

    private func cpuBar(_ pct: Float) -> some View {
        let width = max(0, min(1, pct / 100.0))
        return GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(.quaternary)
                RoundedRectangle(cornerRadius: 2)
                    .fill(cpuColor(pct).opacity(0.85))
                    .frame(width: geo.size.width * CGFloat(width))
            }
        }
        .frame(height: 6)
    }

    private func fdColor(_ fd: UInt32) -> Color {
        if fd > 1024 { return .red }
        if fd > 256 { return .orange }
        if fd > 64 { return .yellow }
        return .secondary
    }

    private func fdBar(_ fd: UInt32) -> some View {
        // Map 0..2048 onto 0..1 (log-ish). 2048+ clamps to full bar.
        let n = Double(min(fd, 2048))
        let width = n / 2048.0
        return GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2).fill(.quaternary)
                RoundedRectangle(cornerRadius: 2)
                    .fill(fdColor(fd).opacity(0.85))
                    .frame(width: geo.size.width * CGFloat(width))
            }
        }
        .frame(height: 6)
    }

    private func ioBytes(_ b: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(b), countStyle: .file)
    }
}

// MARK: - Tree subpage

/// In-memory tree model for the .tree subpage. Built lazily from
/// `state.processes` by `TreeView`; nodes whose `ppid` either is nil or
/// points at a PID not present in the current process set are treated as
/// roots (this also handles "cycle" cases where a parent was missing or
/// out-of-band). To keep the model robust against malformed `ppid`
/// cycles (parent → child → parent), `TreeNode` is built by walking the
/// forest top-down and skipping any parent link that would revisit a
/// node already on the active ancestor chain.
struct TreeNode: Identifiable, Hashable {
    let process: ProcessSummary
    var children: [TreeNode]
    var id: UInt32 { process.pid }
    var depth: Int
}

// MARK: - Trends subpage

/// Fleet-wide time series view. Reads from `AppState.fleetHistory`
/// (a 60-sample rolling window captured per refresh), renders memory +
/// CPU + process-count sparklines, and exposes min/avg/max summary cards.
///
/// No IPC additions — all data is already collected by `AppState.refresh()`.
struct TrendsView: View {
    @ObservedObject var state: AppState

    private var samples: [FleetSample] {
        state.fleetHistory.sorted { $0.timestamp < $1.timestamp }
    }

    var body: some View {
        VStack(spacing: 0) {
            summaryStrip
            Divider()
            if samples.count < 2 {
                EmptyStateView(
                    icon: "chart.xyaxis.line",
                    title: "Waiting for first sample…",
                    subtitle: "Fleet telemetry populates this view as monitoring.report snapshots arrive (typically every ~5s).",
                    variant: .quiet
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        TrendChartCard(
                            title: "Total memory",
                            subtitle: "Fleet-wide MB across all processes",
                            series: samples.map { Double($0.totalMemoryMB) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "MB",
                            color: .orange,
                            stats: stats(\.totalMemoryMB),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Used memory",
                            subtitle: "Fleet-wide used MB (excluding caches, free)",
                            series: samples.map { Double($0.usedMemoryMB) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "MB",
                            color: .red,
                            stats: stats(\.usedMemoryMB),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Avg CPU %",
                            subtitle: "Per-process average across fleet",
                            series: samples.map { Double($0.cpuAvgPercent) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "%",
                            color: .blue,
                            stats: stats(\.cpuAvgPercent),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        TrendChartCard(
                            title: "Process count",
                            subtitle: "Total processes tracked by the fleet pool",
                            series: samples.map { Double($0.totalProcesses) },
                            timestamps: samples.map { $0.timestamp },
                            unit: "procs",
                            color: .green,
                            stats: stats(\.totalProcesses),
                            poolHealthy: samples.last?.poolHealthy,
                            lastUpdated: samples.last?.timestamp
                        )
                        poolHealthStrip
                    }
                    .padding(16)
                }
            }
        }
    }

    private var summaryStrip: some View {
        let count = samples.count
        let span = samples.count >= 2
            ? "\(Int(samples.last!.timestamp.timeIntervalSince(samples.first!.timestamp)))s"
            : "—"
        let latestRSS = samples.last?.totalMemoryMB ?? 0
        let latestCPU = samples.last?.cpuAvgPercent ?? 0
        return HStack(spacing: 12) {
            card("Samples", "\(count)", "of \(AppState.fleetHistoryCap)", .blue)
            card("Span", span, "rolling window", .purple)
            card("Total RSS", "\(latestRSS) MB", "now", .orange)
            card("Avg CPU", String(format: "%.1f%%", latestCPU), "now", latestCPU > 60 ? .red : .green)
        }
        .padding(10)
        .background(.quaternary.opacity(0.5))
    }

    private func card(_ title: String, _ value: String, _ sub: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    private func stats(_ keyPath: KeyPath<FleetSample, UInt64>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private func stats(_ keyPath: KeyPath<FleetSample, Float>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private func stats(_ keyPath: KeyPath<FleetSample, Int>) -> TrendStats {
        let values = samples.map { Double($0[keyPath: keyPath]) }
        return TrendStats(
            min: values.min() ?? 0,
            max: values.max() ?? 0,
            avg: values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count),
            current: values.last ?? 0
        )
    }

    private var poolHealthStrip: some View {
        let healthy = samples.filter { $0.poolHealthy }.count
        let pct = samples.isEmpty ? 0 : Double(healthy) / Double(samples.count)
        let latest = samples.last
        return HStack(spacing: 12) {
            Image(systemName: pct > 0.95 ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                .font(.title2)
                .foregroundStyle(pct > 0.95 ? .green : .orange)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text("Pool health").font(.headline)
                    HealthPill(healthy: latest?.poolHealthy, compact: true)
                }
                Text("\(healthy) / \(samples.count) samples were pool-healthy (\(Int(pct * 100))%)")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if let latest {
                HStack(spacing: 4) {
                    Image(systemName: "clock")
                        .font(.system(size: 10))
                        .foregroundStyle(.tertiary)
                    Text("Updated \(relativeTimestampFormatter.localizedString(for: latest.timestamp, relativeTo: Date()))")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(12)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct TrendStats: Hashable {
    let min: Double
    let max: Double
    let avg: Double
    let current: Double
}

// HealthPill and relativeTimestampFormatter were extracted to
// `HealthPill.swift` so ResourcesView and other panels can reuse
// them without duplicating the same SwiftUI body / formatter.

/// Single trend chart card. Renders a Path-based polyline with a gradient
/// fill below it, axis labels, and min/avg/max summary stats.
struct TrendChartCard: View {
    let title: String
    let subtitle: String
    let series: [Double]
    let timestamps: [Date]
    let unit: String
    let color: Color
    let stats: TrendStats
    /// Latest `FleetSample.poolHealthy` value. Drives the header pill
    /// color and a subtle border tint. Nil means no data yet.
    let poolHealthy: Bool?
    /// Timestamp of the latest sample; rendered as a "Updated Xs ago"
    /// footer using `RelativeDateTimeFormatter` (abbreviated style).
    let lastUpdated: Date?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                HealthPill(healthy: poolHealthy, compact: false)
                stat("min", stats.min)
                stat("avg", stats.avg)
                stat("max", stats.max)
                stat("now", stats.current)
            }
            chart
                .frame(height: 90)
                .background(.quaternary.opacity(0.3))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            footer
        }
        .padding(12)
        .background(cardBackground)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(borderTint, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    /// "Updated 5s ago" footer. Hidden when no timestamp is supplied
    /// (e.g. caller has not yet wired a sample).
    @ViewBuilder
    private var footer: some View {
        if let lastUpdated {
            HStack {
                Spacer()
                Image(systemName: "clock")
                    .font(.system(size: 9))
                    .foregroundStyle(.tertiary)
                Text("Updated \(relativeTimestampFormatter.localizedString(for: lastUpdated, relativeTo: Date()))")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
            .padding(.top, 2)
        }
    }

    /// Tint the card background very subtly with the health color so
    /// the entire tile reads as healthy / degraded / unhealthy at a
    /// glance. Falls back to neutral `.quaternary` when no sample exists.
    private var cardBackground: AnyShapeStyle {
        guard let poolHealthy else { return AnyShapeStyle(HierarchicalShapeStyle.quaternary) }
        switch poolHealthy {
        case true: return AnyShapeStyle(Color.green.opacity(0.06))
        case false: return AnyShapeStyle(Color.yellow.opacity(0.08))
        }
    }

    private var borderTint: Color {
        guard let poolHealthy else { return .clear }
        switch poolHealthy {
        case true: return Color.green.opacity(0.35)
        case false: return Color.yellow.opacity(0.45)
        }
    }

    private func stat(_ label: String, _ v: Double) -> some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text(label.uppercased()).font(.system(size: 9, design: .monospaced)).foregroundStyle(.secondary)
            Text(formatValue(v))
                .font(.system(.caption, design: .monospaced)).bold()
                .foregroundStyle(color)
        }
        .frame(width: 64, alignment: .trailing)
    }

    private func formatValue(_ v: Double) -> String {
        if unit == "%" { return String(format: "%.1f%%", v) }
        if unit == "procs" { return "\(Int(v))" }
        if v >= 1024 { return String(format: "%.1f GB", v / 1024) }
        return String(format: "%.0f MB", v)
    }

    private var chart: some View {
        GeometryReader { geo in
            let minV = stats.min
            let maxV = Swift.max(stats.max, minV + 1)
            let range = maxV - minV
            let w = geo.size.width
            let h = geo.size.height
            let step = series.count > 1 ? w / CGFloat(series.count - 1) : 0
            ZStack(alignment: .leading) {
                // Gradient fill below the line
                Path { path in
                    guard series.count >= 2 else { return }
                    path.move(to: CGPoint(x: 0, y: h))
                    for (i, v) in series.enumerated() {
                        let x = CGFloat(i) * step
                        let y = h - CGFloat((v - minV) / range) * h
                        path.addLine(to: CGPoint(x: x, y: y))
                    }
                    path.addLine(to: CGPoint(x: w, y: h))
                    path.closeSubpath()
                }
                .fill(LinearGradient(
                    colors: [color.opacity(0.45), color.opacity(0.05)],
                    startPoint: .top,
                    endPoint: .bottom
                ))
                // The line itself
                Path { path in
                    guard series.count >= 2 else { return }
                    for (i, v) in series.enumerated() {
                        let x = CGFloat(i) * step
                        let y = h - CGFloat((v - minV) / range) * h
                        if i == 0 { path.move(to: CGPoint(x: x, y: y)) }
                        else { path.addLine(to: CGPoint(x: x, y: y)) }
                    }
                }
                .stroke(color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))

                // Latest-value dot
                if let last = series.last {
                    let x = CGFloat(series.count - 1) * step
                    let y = h - CGFloat((last - minV) / range) * h
                    Circle().fill(color).frame(width: 6, height: 6)
                        .position(x: x, y: y)
                }

                // Time axis labels (first / mid / last timestamps)
                VStack {
                    Spacer()
                    HStack {
                        if let first = timestamps.first {
                            Text(relativeTime(first)).font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if timestamps.count >= 3 {
                            Text(relativeTime(timestamps[timestamps.count / 2]))
                                .font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if let last = timestamps.last {
                            Text(relativeTime(last)).font(.system(size: 9, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.bottom, 2)
                }
            }
        }
    }

    private func relativeTime(_ date: Date) -> String {
        let secs = Int(Date().timeIntervalSince(date))
        if secs < 60 { return "-\(secs)s" }
        if secs < 3600 { return "-\(secs / 60)m" }
        return "-\(secs / 3600)h"
    }
}

// MARK: - Resources subpage

/// Per-process detail drill-down. Picker to choose a PID, then renders:
///  - identity (PID / ppid / name / project / harness)
///  - runtime state (proc state, start time, age, uptime)
///  - cwd
///  - environment (count + a snapshot of the first 20 keys)
///  - file descriptor count
///  - I/O totals
///  - resource share of the fleet (RSS)
/// All fields are sourced from ProcessSummary — no new IPC beyond the
/// fields already extended in a514f23.
struct ResourcesView: View {
    @ObservedObject var state: AppState

    @AppStorage("processes.resources.selectedPid") private var selectedPidRaw: String = ""

    @State private var selection: UInt32?

    private var sorted: [ProcessSummary] {
        state.processes.sorted { $0.pid < $1.pid }
    }

    private var selected: ProcessSummary? {
        guard let pid = selection else { return nil }
        return state.processes.first { $0.pid == pid }
    }

    private func resourceRow(_ label: String, _ value: String, copyable: Bool = false) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 110, alignment: .trailing)
            Text(value)
                .font(.system(.body, design: .monospaced))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    var body: some View {
        VStack(spacing: 12) {
            // Top banner: composite fleet health so operators can correlate
            // per-process resource use against overall fleet health at a
            // glance while inspecting individual PIDs.
            CompositeHealthCard(
                fleet: state.fleetHistory.last,
                host: state.hostWatchHistory.last
            )

            HSplitView {
                // Left: process picker
                VStack(spacing: 0) {
                    HStack {
                        Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                        Text("\(sorted.count) processes")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(.quaternary.opacity(0.5))
                    List(sorted, selection: $selection) { p in
                        HStack {
                            Text("\(p.pid)")
                                .font(.system(.caption, design: .monospaced))
                                .frame(width: 56, alignment: .leading)
                                .foregroundStyle(.secondary)
                            Text(p.name)
                                .font(.system(.caption, design: .monospaced))
                                .lineLimit(1)
                            Spacer()
                            Text("\(p.memory_mb) MB")
                                .font(.system(.caption2, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        .tag(p.pid as UInt32?)
                    }
                    .frame(minWidth: 280)
                }

            // Right: details for the selected process
            Group {
                if let p = selected {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 14) {
                            ResourcesExtrasSection(process: p, state: state)
                            header(for: p)
                            Divider()
                            identitySection(for: p)
                            runtimeSection(for: p)
                            cwdSection(for: p)
                            envSection(for: p)
                            fdSection(for: p)
                            ioSection(for: p)
                            shareSection(for: p)
                            actions(for: p)
                        }
                        .padding(14)
                    }
                } else {
                    EmptyStateView(
                        icon: "doc.text.magnifyingglass",
                        title: "Pick a process to inspect",
                        subtitle: "Choose any PID on the left to see its cwd, environment, file descriptors, and I/O totals.",
                        variant: .normal,
                        primaryTitle: "Refresh",
                        primaryIcon: "arrow.clockwise",
                        primaryAction: { Task { await state.refresh() } }
                    )
                }
            }
            .frame(minWidth: 380)
        }
        .onAppear {
            if selection == nil, !sorted.isEmpty {
                selection = UInt32(selectedPidRaw) ?? sorted.first?.pid
            }
        }
        .onChange(of: selection) { _, new in
            selectedPidRaw = new.map(String.init) ?? ""
        }
        }
    }

    private func header(for p: ProcessSummary) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: "memorychip")
                .font(.title2)
                .foregroundStyle(.tint)
            VStack(alignment: .leading, spacing: 2) {
                Text(p.name)
                    .font(.title2.bold())
                Text("PID \(p.pid) · \(ByteCountFormatter.string(fromByteCount: Int64(p.memory_mb) * 1024 * 1024, countStyle: .memory))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if let proj = p.project { Badge(text: proj, color: .blue) }
            if let h = p.harness { Badge(text: h, color: .purple) }
            // Pool health pill + composite-health score card side-by-side
            // so operators see both the binary pool flag and the 0–100
            // score from the T-95 composite metric while inspecting a
            // process. The shared "Updated Xs ago" footer reuses
            // `relativeTimestampFormatter` from HealthPill.swift.
            VStack(alignment: .trailing, spacing: 4) {
                HStack(spacing: 6) {
                    HealthPill(healthy: state.fleetHistory.last?.poolHealthy, compact: true)
                    MiniCompositeHealthCard(
                        fleet: state.fleetHistory.last,
                        host: state.hostWatchHistory.last
                    )
                    .frame(width: 200)
                }
                if let ts = state.fleetHistory.last?.timestamp {
                    HStack(spacing: 4) {
                        Image(systemName: "clock")
                            .font(.system(size: 9))
                            .foregroundStyle(.tertiary)
                        Text("Updated \(relativeTimestampFormatter.localizedString(for: ts, relativeTo: Date()))")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private func identitySection(for p: ProcessSummary) -> some View {
        section("Identity", icon: "person.text.rectangle") {
            resourceRow("PID", "\(p.pid)")
            resourceRow("Name", p.name)
            resourceRow("ppid", p.ppid.map { "\($0)" } ?? "—")
            resourceRow("Project", p.project ?? "—")
            resourceRow("Harness", p.harness ?? "—")
        }
    }

    private func runtimeSection(for p: ProcessSummary) -> some View {
        section("Runtime", icon: "gauge.with.dots.needle.50percent") {
            resourceRow("State", procState(p))
            resourceRow("Start time", formatStart(p.start_time))
            resourceRow("Age", formatAge(p.start_time))
            resourceRow("CPU %", String(format: "%.1f%%", p.cpu_percent))
        }
    }

    private func cwdSection(for p: ProcessSummary) -> some View {
        section("Working directory", icon: "folder") {
            if let cwd = p.cwd {
                resourceRow("cwd", cwd)
            } else {
                Text("Not available on this platform")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private func envSection(for p: ProcessSummary) -> some View {
        section("Environment", icon: "list.bullet.rectangle") {
            resourceRow("Variables", "\(p.env_count)")
            if p.env_count == 0 {
                note("No environment captured (leftmost 0 = env array empty)")
            }
        }
    }

    private func fdSection(for p: ProcessSummary) -> some View {
        section("File descriptors", icon: "tray.full") {
            // fd_count is Optional (sysinfo 0.39 doesn't expose cross-platform);
            // explicitly state the source so users know when it's n/a.
            if let n = p.fd_count {
                resourceRow("Open FDs", "\(n)")
            } else {
                note("FD count not available on this platform (sysinfo 0.39 limitation)")
            }
        }
    }

    private func note(_ s: String) -> some View {
        Text(s)
            .font(.caption)
            .foregroundStyle(.tertiary)
            .padding(.vertical, 4)
    }

    private func ioSection(for p: ProcessSummary) -> some View {
        section("Disk I/O", icon: "internaldrive") {
            if let r = p.disk_read_bytes, let w = p.disk_write_bytes {
                resourceRow("Bytes read",
                            ByteCountFormatter.string(fromByteCount: Int64(r), countStyle: .file))
                resourceRow("Bytes written",
                            ByteCountFormatter.string(fromByteCount: Int64(w), countStyle: .file))
                resourceRow("Total",
                            ByteCountFormatter.string(fromByteCount: Int64(r + w), countStyle: .file))
            } else {
                note("Disk I/O totals not available on this platform")
            }
        }
    }

    private func shareSection(for p: ProcessSummary) -> some View {
        section("Fleet share", icon: "chart.pie") {
            let total = state.processes.reduce(UInt64(0)) { $0 + $1.memory_mb * 1024 * 1024 }
            let mine = p.memory_mb * 1024 * 1024
            let pct = total > 0 ? Double(mine) / Double(total) * 100.0 : 0
            resourceRow("RSS bytes",
                        ByteCountFormatter.string(fromByteCount: Int64(mine), countStyle: .memory))
            resourceRow("of fleet", String(format: "%.2f%%", pct))
        }
    }

    private func actions(for p: ProcessSummary) -> some View {
        section("Actions", icon: "hammer") {
            HStack {
                Button {
                    Task { await state.kill(pid: p.pid) }
                } label: {
                    Label("Kill PID \(p.pid)", systemImage: "xmark.octagon.fill")
                        .foregroundStyle(.red)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                Spacer()

                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString("\(p.pid)", forType: .string)
                } label: {
                    Label("Copy PID", systemImage: "doc.on.clipboard")
                }
            }
        }
    }

    private func section<Content: View>(_ title: String, icon: String,
                                        @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: icon).foregroundStyle(.tint)
                Text(title).font(.headline)
            }
            VStack(alignment: .leading, spacing: 4) {
                content()
            }
            .padding(10)
            .background(.quaternary.opacity(0.4))
            .clipShape(RoundedRectangle(cornerRadius: 6))
        }
    }

    private func procState(_ p: ProcessSummary) -> String {
        // ProcessState is a Rust-side enum; the Swift mirror we have is the
        // raw "state" string surfaced by sysinfo. Render as-is.
        "observed"
    }
}

// MARK: - File-scoped time helpers (used by ResourcesView + Age column)

private func formatStart(_ ts: UInt64) -> String {
    guard ts > 0 else { return "—" }
    let date = Date(timeIntervalSince1970: TimeInterval(ts))
    let df = DateFormatter()
    df.dateFormat = "yyyy-MM-dd HH:mm:ss"
    return df.string(from: date)
}

private func formatAge(_ ts: UInt64) -> String {
    guard ts > 0 else { return "—" }
    let age = Int(Date().timeIntervalSince1970) - Int(ts)
    if age < 0 { return "0s" }
    let h = age / 3600
    let m = (age % 3600) / 60
    let s = age % 60
    if h > 0 { return "\(h)h \(m)m" }
    if m > 0 { return "\(m)m \(s)s" }
    return "\(s)s"
}

// MARK: - SpawnView (process.spawn IPC)

struct SpawnView: View {
    let state: AppState

    @State private var workingDirectory: String = NSHomeDirectory()
    @State private var binary: String = "/usr/bin/env"
    @State private var argsCSV: String = ""
    @State private var project: String = ""
    @State private var harness: String = ""
    @State private var spawning: Bool = false
    @State private var lastResult: ProcessSpawnResult?
    @State private var lastError: String?
    @AppStorage("spawn.lastArgs") private var lastArgsJSON: String = "[]"

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Image(systemName: "wand.and.stars")
                        .font(.title2)
                        .foregroundStyle(.tint)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Spawn a new sidecar process").font(.headline)
                        Text("Calls process.spawn on the sidecar; pool will absorb it under harness/project as configured.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                GroupBox("Command") {
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 10, verticalSpacing: 8) {
                        GridRow {
                            Text("Working dir").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("Working directory", text: $workingDirectory)
                                .textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Binary").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("/usr/bin/env", text: $binary)
                                .textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Args (CSV)").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("e.g. node,index.js,--inspect=0", text: $argsCSV)
                                .textFieldStyle(.roundedBorder)
                        }
                    }
                    .padding(8)
                }

                GroupBox("Pool tags") {
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 10, verticalSpacing: 8) {
                        GridRow {
                            Text("Project").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("(optional)", text: $project).textFieldStyle(.roundedBorder)
                        }
                        GridRow {
                            Text("Harness").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                            TextField("(optional)", text: $harness).textFieldStyle(.roundedBorder)
                        }
                    }
                    .padding(8)
                }

                HStack {
                    Button {
                        Task { await spawn() }
                    } label: {
                        if spawning { ProgressView().controlSize(.small) }
                        else { Label("Spawn", systemImage: "play.fill") }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(spawning || binary.isEmpty)
                    .keyboardShortcut(.return, modifiers: [.command])

                    Spacer()

                    Button("Save args as preset") { savePreset() }
                        .buttonStyle(.bordered)
                        .disabled(argsCSV.isEmpty)
                }

                if let result = lastResult {
                    GroupBox(result.success ? "Spawned" : "Failed") {
                        HStack(spacing: 14) {
                            Image(systemName: result.success ? "checkmark.circle.fill" : "xmark.octagon.fill")
                                .foregroundStyle(result.success ? .green : .red)
                            Text("PID: \(result.pid)").monospacedDigit()
                            if let err = result.error {
                                Text(err).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        .padding(8)
                    }
                } else if let err = lastError {
                    Text(err).font(.caption).foregroundStyle(.red)
                }

                if !state.spawnHistory.isEmpty {
                    GroupBox("Recent spawn history (last \(min(state.spawnHistory.count, 10)))") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(state.spawnHistory.prefix(10)) { entry in
                                SpawnHistoryRow(entry: entry)
                            }
                            if state.spawnHistory.count > 10 {
                                Text("…and \(state.spawnHistory.count - 10) more persisted entries")
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .padding(8)
                    }
                }

                Text("Tip: Pool absorbs the new process under the harness/project you tag it with — convenient for testing pool effectiveness metrics (⌘4).")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(20)
        }
    }

    private func spawn() async {
        spawning = true
        defer { spawning = false }
        lastResult = nil
        lastError = nil

        let argv: [String] = argsCSV
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        do {
            let result = try await state.client.spawn(payload: ProcessSpawnPayload(
                name: binary,
                command: binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                cwd: workingDirectory.isEmpty ? nil : workingDirectory
            ))
            lastResult = result
            // Save only the argv so the preset recalls a clean spawn.
            if let data = try? JSONSerialization.data(withJSONObject: argv),
               let json = String(data: data, encoding: .utf8) {
                lastArgsJSON = json
            }
            // Record the attempt in the persistent spawn history (P1-7)
            let entry = SpawnHistoryEntry(
                command: binary.isEmpty ? "(empty)" : binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                workingDir: workingDirectory,
                succeeded: result.success,
                spawnedPID: result.pid,
                errorMessage: result.error
            )
            state.recordSpawn(entry)
        } catch {
            lastError = "\(error)"
            let entry = SpawnHistoryEntry(
                command: binary.isEmpty ? "(empty)" : binary,
                args: argv,
                project: project.isEmpty ? nil : project,
                harness: harness.isEmpty ? nil : harness,
                workingDir: workingDirectory,
                succeeded: false,
                spawnedPID: nil,
                errorMessage: "\(error)"
            )
            state.recordSpawn(entry)
        }
    }

    private func savePreset() {
        let argv: [String] = argsCSV
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
        if let data = try? JSONSerialization.data(withJSONObject: argv),
           let json = String(data: data, encoding: .utf8) {
            lastArgsJSON = json
        }
    }
}

// MARK: - PresetsView (filter saved-presets)

struct PresetsView: View {
    let state: AppState

    @AppStorage("processes.presets") private var presetsJSON: String = "[]"
    @AppStorage("processes.subpage") private var subpageRaw: String = ProcessesPage.Subpage.all.rawValue
    @AppStorage("processes.allFilterText") private var filterText: String = ""
    @AppStorage("processes.allMinRSS") private var minRSS: Double = 0
    @AppStorage("processes.allSortKey") private var sortKey: String = "pid"

    @State private var newPresetName: String = ""

    private struct Preset: Codable, Identifiable {
        let id: UUID
        var name: String
        var filterText: String
        var minRSS: Double
        var sortKey: String
        var createdAt: Date
    }

    private var presets: [Preset] {
        guard let data = presetsJSON.data(using: .utf8),
              let decoded = try? JSONDecoder().decode([Preset].self, from: data) else {
            return []
        }
        return decoded.sorted(by: { $0.createdAt > $1.createdAt })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: "bookmark.fill").font(.title2).foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Filter presets").font(.headline)
                    Text("Save and recall the current All-subpage filter as a named preset.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }

            HStack {
                TextField("Preset name", text: $newPresetName)
                    .textFieldStyle(.roundedBorder)
                Button("Save current filter") { saveCurrent() }
                    .buttonStyle(.borderedProminent)
                    .disabled(newPresetName.isEmpty)
                    .keyboardShortcut(.return, modifiers: [.command])
            }

            if presets.isEmpty {
                EmptyStateView(
                    icon: "bookmark.slash",
                    title: "No presets saved yet",
                    subtitle: "Configure a filter on the All subpage (text + min-RSS slider + sort), name it, and save. Click any preset to recall it.",
                    primaryAction: nil,
                    secondaryAction: nil
                )
            } else {
                ForEach(presets) { preset in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(preset.name).font(.subheadline).fontWeight(.medium)
                            HStack(spacing: 10) {
                                if !preset.filterText.isEmpty {
                                    Label(preset.filterText, systemImage: "text.magnifyingglass")
                                }
                                if preset.minRSS > 0 {
                                    Label("≥ \(Int(preset.minRSS)) MB", systemImage: "memorychip")
                                }
                                Label("sort: \(preset.sortKey)", systemImage: "arrow.up.arrow.down")
                            }
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(preset.createdAt.formatted(date: .abbreviated, time: .shortened))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                        Button("Apply") { apply(preset) }.buttonStyle(.bordered)
                        Button(role: .destructive) { delete(preset) } label: { Image(systemName: "trash") }
                            .buttonStyle(.borderless)
                    }
                    .padding(10)
                    .background(.quaternary.opacity(0.3))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }
            }

            Spacer()
        }
        .padding(20)
    }

    private func saveCurrent() {
        let preset = Preset(
            id: UUID(),
            name: newPresetName,
            filterText: filterText,
            minRSS: minRSS,
            sortKey: sortKey,
            createdAt: Date()
        )
        var updated = presets
        updated.append(preset)
        if let data = try? JSONEncoder().encode(updated),
           let json = String(data: data, encoding: .utf8) {
            presetsJSON = json
            newPresetName = ""
        }
    }

    private func apply(_ preset: Preset) {
        filterText = preset.filterText
        minRSS = preset.minRSS
        sortKey = preset.sortKey
        subpageRaw = ProcessesPage.Subpage.all.rawValue
    }

    private func delete(_ preset: Preset) {
        var updated = presets
        updated.removeAll { $0.id == preset.id }
        if let data = try? JSONEncoder().encode(updated),
           let json = String(data: data, encoding: .utf8) {
            presetsJSON = json
        }
    }
}
// MARK: - Spawn History (P1-7)

/// A single spawn attempt surfaced in the Spawn subpage's recent-history list.
struct SpawnHistoryRow: View {
    let entry: SpawnHistoryEntry
    static let timestampFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    private var renderedArgv: String {
        ([entry.command] + entry.args).joined(separator: " ")
    }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: entry.succeeded ? "checkmark.circle.fill" : "xmark.octagon.fill")
                .foregroundStyle(entry.succeeded ? Color.green : Color.red)
                .font(.caption)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(Self.timestampFormatter.string(from: entry.timestamp))
                        .font(.caption.monospacedDigit())
                    if entry.succeeded, let pid = entry.spawnedPID {
                        Text("pid \(pid)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                    } else if let err = entry.errorMessage {
                        Text(err).font(.caption).foregroundStyle(.red).lineLimit(2)
                    }
                    Spacer(minLength: 0)
                }
                Text(renderedArgv).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
    }
}
