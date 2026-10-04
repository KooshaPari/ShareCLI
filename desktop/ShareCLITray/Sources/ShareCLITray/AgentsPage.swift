/// AgentsPage.swift — dedicated page for "found agents" surfaced from the
/// fleet monitoring `agents` array (decoded into `StatusSnapshot.agents`).
///
/// This is PR 1 of the dashboard expansion plan (`plans/2026-07-25-tray-dashboard-expanded-v1.md`).
/// No sidecar IPC additions are needed — the data is already in the
/// `monitoring.report` envelope, fully decoded into `AppState.statusSnapshot.agents`.
///
/// Layout:
///   ┌────────────────────────────────┬───────────────────────────────┐
///   │ Agents List (sortable Table)   │ Agent Detail (selected row)   │
///   │  • filter by family            │  • PID / family / comm / state │
///   │  • sort by RSS / PID / family  │  • memory rss (human + bytes) │
///   │  • kill row action             │  • fd_count + state badge     │
///   │  • summary strip (totals)      │  • placeholder sparkline      │
///   └────────────────────────────────┴───────────────────────────────┘

import SwiftUI
import ShareCLICore

struct AgentsPage: View {
    @ObservedObject var state: AppState

    /// Reduce motion (PLAN 1.23): gates the numeric-value tween below.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @AppStorage("agents.selectedFamily") private var selectedFamilyFilter: String = "all"
    @AppStorage("agents.selectedPID") private var selectedPID: Int = 0
    @State private var filterText: String = ""
    @FocusState private var filterFocused: Bool
    @State private var sortOrder: [KeyPathComparator<AgentProcRow>] = [
        KeyPathComparator(\AgentProcRow.mem_rss_bytes, order: .reverse)
    ]

    private var allAgents: [AgentProcRow] {
        state.statusSnapshot?.agents ?? []
    }

    private var families: [String] {
        var seen = Set<String>()
        var ordered: [String] = []
        for a in allAgents {
            if seen.insert(a.family).inserted {
                ordered.append(a.family)
            }
        }
        return ordered.sorted()
    }

    private var filtered: [AgentProcRow] {
        let q = filterText.lowercased()
        return allAgents
            .filter { agent in
                if selectedFamilyFilter != "all" && agent.family != selectedFamilyFilter {
                    return false
                }
                if q.isEmpty { return true }
                return agent.comm.lowercased().contains(q)
                    || agent.family.lowercased().contains(q)
                    || agent.state.lowercased().contains(q)
                    || String(agent.pid).contains(q)
            }
            .sorted(using: sortOrder)
    }

    private var selectedAgent: AgentProcRow? {
        guard selectedPID > 0 else { return nil }
        return allAgents.first { $0.pid == UInt32(selectedPID) }
    }

    var body: some View {
        HSplitView {
            agentsList
                .frame(minWidth: 380, idealWidth: 480)
            agentDetail
                .frame(minWidth: 320, idealWidth: 360)
        }
        .frame(minWidth: 720, minHeight: 420)
        .toolbar {
            ToolbarItem {
                Picker("Family", selection: $selectedFamilyFilter) {
                    Text("All families").tag("all")
                    ForEach(families, id: \.self) { fam in
                        Text(fam).tag(fam)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
            }
        }
    }

    // MARK: - Agents List

    private var agentsList: some View {
        VStack(spacing: 0) {
            summaryStrip
            filterBar
            if allAgents.isEmpty {
                EmptyStateView(
                    icon: "person.2.slash",
                    title: "No agents observed yet",
                    subtitle: "The /proc scanner hasn't detected any host agents. Once a Claude, Forge, Node, Bun, or other family-compatible process is running on the host, it'll appear here automatically.",
                    variant: .hero,
                    primaryTitle: "Refresh now",
                    primaryIcon: "arrow.clockwise",
                    primaryAction: { Task { await state.refresh() } },
                    secondaryTitle: "What are agents?",
                    secondaryIcon: "questionmark.circle",
                    secondaryAction: {
                        if let url = URL(string: "https://docs.sharecli.dev/agents") { NSWorkspace.shared.open(url) }
                    }
                )
            } else {
                agentsTable
            }
        }
    }

    private var summaryStrip: some View {
        let total = allAgents.count
        let filteredCount = filtered.count
        let totalRSS = allAgents.reduce(UInt64(0)) { $0 + $1.mem_rss_bytes }
        let runningCount = allAgents.filter { $0.state.lowercased().contains("run") }.count

        return HStack(spacing: 16) {
            summaryCard(
                title: "Found",
                value: "\(total)",
                sub: filteredCount == total ? "agents" : "\(filteredCount) shown",
                color: .blue
            )
            .animateInOnAppear(delay: 0.0)
            .hoverGlow()
            summaryCard(
                title: "Running",
                value: "\(runningCount)",
                sub: "in state *run*",
                color: .green
            )
            .animateInOnAppear(delay: 0.06)
            .hoverGlow()
            summaryCard(
                title: "Total RSS",
                value: ByteCountFormatter.string(fromByteCount: Int64(totalRSS), countStyle: .memory),
                sub: "across all agents",
                color: .orange
            )
            .animateInOnAppear(delay: 0.12)
            .hoverGlow()
            summaryCard(
                title: "Families",
                value: "\(families.count)",
                sub: families.prefix(3).joined(separator: ", ")
                    + (families.count > 3 ? "…" : ""),
                color: .purple
            )
            .animateInOnAppear(delay: 0.18)
            .hoverGlow()
        }
        .padding(10)
        .background(.quaternary.opacity(0.5))
    }

    private func summaryCard(title: String, value: String, sub: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption2).foregroundStyle(.secondary)
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
                .contentTransition(Motion.isEnabled(reduceMotion: reduceMotion) ? .numericText() : .identity)
                .animation(Motion.animation(.easeOut(duration: 0.32), reduceMotion: reduceMotion), value: value)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    private var filterBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Filter by comm / family / state / PID", text: $filterText)
                .textFieldStyle(.plain)
                .filterFieldFocus($filterFocused)
            if !filterText.isEmpty {
                Button {
                    filterText = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
            }
            Spacer()
            Text("\(filtered.count) / \(allAgents.count)")
                .font(.caption).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.3))
    }

    private var agentsTable: some View {
        Table(filtered, selection: Binding(
            get: { selectedPID > 0 ? UInt32(selectedPID) : nil },
            set: { newValue in
                selectedPID = newValue.map { Int($0) } ?? 0
            }
        ), sortOrder: $sortOrder) {
            TableColumn("PID", value: \.pid) { agent in
                Text("\(agent.pid)").font(.system(.body, design: .monospaced))
            }
            .width(60)

            TableColumn("Family", value: \.family) { agent in
                Badge(text: agent.family, color: .purple)
            }
            .width(90)

            TableColumn("Comm", value: \.comm) { agent in
                Text(agent.comm).font(.system(.body, design: .monospaced)).lineLimit(1)
            }

            TableColumn("State", value: \.state) { agent in
                Badge(text: agent.state, color: stateColor(agent.state))
            }
            .width(80)

            TableColumn("RSS", value: \.mem_rss_bytes) { agent in
                Text(agent.mem_rss)
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(rssColor(agent.mem_rss_bytes))
            }
            .width(80)

            TableColumn("FDs", value: \.fd_count, comparator: OptionalIntComparator()) { agent in
                if let fds = agent.fd_count {
                    Text("\(fds)").font(.system(.body, design: .monospaced))
                } else {
                    Text("—").foregroundStyle(.secondary)
                }
            }
            .width(50)

            TableColumn("Actions") { agent in
                Button {
                    Task { await state.kill(pid: agent.pid) }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.red)
                }
                .buttonStyle(.borderless)
                .help("Kill PID \(agent.pid) (\(agent.comm))")
            }
            .width(40)
        }
    }

    // MARK: - Agent Detail

    private var agentDetail: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let agent = selectedAgent {
                    detailHeader(agent)
                    detailFields(agent)
                    detailRSSChart(agent)
                    detailCmdline(agent)
                    detailActions(agent)
                } else {
                    VStack(spacing: 8) {
                        Image(systemName: "person.crop.circle.dashed")
                            .font(.system(size: 36))
                            .foregroundStyle(.secondary)
                        Text("Select an agent")
                            .foregroundStyle(.secondary)
                        Text("Pick a row from the table to inspect pid, family, comm, state, memory, fd counts, and command-line.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 280)
                    }
                    .frame(maxWidth: .infinity, minHeight: 240)
                    .padding(24)
                }
            }
            .padding(16)
        }
        .background(.background)
        .onChange(of: selectedPID) { _, newValue in
            // Trigger cmdline fetch whenever the user picks a new agent.
            if newValue > 0 {
                Task { _ = await state.fetchCmdlineIfNeeded(pid: UInt32(newValue)) }
            }
        }
        .task(id: selectedPID) {
            if selectedPID > 0 {
                _ = await state.fetchCmdlineIfNeeded(pid: UInt32(selectedPID))
            }
        }
    }

    private func detailHeader(_ agent: AgentProcRow) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(agent.comm)
                    .font(.system(.title2, design: .monospaced))
                    .bold()
                Spacer()
                Badge(text: agent.family, color: .purple)
            }
            Text("PID \(agent.pid) · state \(agent.state)")
                .font(.caption)
                .foregroundStyle(.secondary)
                .font(.system(.caption, design: .monospaced))
        }
    }

    private func detailFields(_ agent: AgentProcRow) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            detailRow("PID", value: "\(agent.pid)")
            detailRow("Family", value: agent.family)
            detailRow("Comm", value: agent.comm, mono: true)
            detailRow("State", value: agent.state, color: stateColor(agent.state))
            detailRow("RSS (human)", value: agent.mem_rss)
            detailRow("RSS (bytes)", value: "\(agent.mem_rss_bytes)", mono: true)
            if let fds = agent.fd_count {
                detailRow("FD count", value: "\(fds)", mono: true)
            }
            detailRow("Last sample", value: "now (live poll)")
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func detailRow(_ label: String, value: String, mono: Bool = false, color: Color = .primary) -> some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 110, alignment: .leading)
            Text(value)
                .font(.system(.body, design: mono ? .monospaced : .default))
                .foregroundStyle(color)
            Spacer()
        }
    }

    private func detailRSSChart(_ agent: AgentProcRow) -> some View {
        let series = state.agentRSSHistory[agent.pid] ?? []
        let values = series.map { Double($0) }
        let minV = values.min() ?? 0
        let maxV = values.max() ?? 1
        let span = String(
            format: "%.1f MB → %.1f MB",
            Double(minV) / 1024.0 / 1024.0,
            Double(maxV) / 1024.0 / 1024.0
        )
        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Memory Series")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(series.count) sample(s) · \(span)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            if series.isEmpty {
                ZStack(alignment: .center) {
                    RoundedRectangle(cornerRadius: 4)
                        .fill(.quaternary)
                        .frame(height: 56)
                    Text("Waiting for next poll…")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            } else {
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 4)
                        .fill(.quaternary)
                        .frame(height: 56)
                    // Plot polyline scaled to the height of the chart.
                    GeometryReader { geo in
                        let path = sparklinePath(
                            values: values,
                            in: CGRect(x: 0, y: 0, width: geo.size.width, height: 56)
                        )
                        path
                            .stroke(rssColor(agent.mem_rss_bytes), lineWidth: 1.5)
                        // Fill under the line.
                        path
                            .fill(LinearGradient(
                                colors: [
                                    rssColor(agent.mem_rss_bytes).opacity(0.35),
                                    rssColor(agent.mem_rss_bytes).opacity(0.0),
                                ],
                                startPoint: .top,
                                endPoint: .bottom
                            ))
                    }
                }
            }
        }
    }

    private func sparklinePath(values: [Double], in rect: CGRect) -> Path {
        var path = Path()
        guard values.count > 1 else {
            if let v = values.first {
                let y = rect.maxY - rect.height * CGFloat((v - minMax(values).0) / max(1, minMax(values).1 - minMax(values).0))
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: rect.maxX, y: y))
            }
            return path
        }
        let (lo, hi) = minMax(values)
        let range = max(1.0, hi - lo)
        let stepX = rect.width / CGFloat(values.count - 1)
        for (i, v) in values.enumerated() {
            let x = CGFloat(i) * stepX
            let y = rect.maxY - rect.height * CGFloat((v - lo) / range)
            if i == 0 {
                path.move(to: CGPoint(x: x, y: y))
            } else {
                path.addLine(to: CGPoint(x: x, y: y))
            }
        }
        return path
    }

    private func minMax(_ values: [Double]) -> (Double, Double) {
        let lo = values.min() ?? 0
        let hi = values.max() ?? 1
        return (lo, hi)
    }

    private func detailCmdline(_ agent: AgentProcRow) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Command line")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                if state.cmdlineCache[agent.pid] == nil {
                    HStack(spacing: 4) {
                        ProgressView().scaleEffect(0.5).frame(width: 12, height: 12)
                        Text("fetching…")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Button {
                        Task { _ = await state.fetchCmdlineIfNeeded(pid: agent.pid) }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                            .font(.caption2)
                    }
                    .buttonStyle(.borderless)
                    .help("Re-fetch command line")
                }
            }
            if let snap = state.cmdlineCache[agent.pid] {
                VStack(alignment: .leading, spacing: 4) {
                    if snap.cmdline.isEmpty {
                        Text("(empty cmdline)")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                    } else {
                        Text(snap.cmdline)
                            .font(.system(.caption, design: .monospaced))
                            .textSelection(.enabled)
                            .lineLimit(6)
                    }
                    if !snap.argv.isEmpty {
                        Text("argv (\(snap.argv.count))")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .padding(.top, 2)
                        ForEach(Array(snap.argv.enumerated()), id: \.offset) { (i, arg) in
                            HStack(alignment: .top, spacing: 6) {
                                Text("[\(i)]")
                                    .font(.system(.caption2, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                    .frame(width: 28, alignment: .leading)
                                Text(arg)
                                    .font(.system(.caption, design: .monospaced))
                                    .textSelection(.enabled)
                            }
                        }
                    }
                }
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary)
                .clipShape(RoundedRectangle(cornerRadius: 6))
            } else {
                Text("Will fetch on selection.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func detailActions(_ agent: AgentProcRow) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                Task { await state.kill(pid: agent.pid) }
            } label: {
                Label("Kill PID \(agent.pid)", systemImage: "xmark.octagon.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
        }
    }

    // MARK: - Helpers

    private func stateColor(_ s: String) -> Color {
        switch s.lowercased() {
        case let x where x.contains("run"): return .green
        case let x where x.contains("sleep"): return .blue
        case let x where x.contains("idle"): return .gray
        case let x where x.contains("zombie") || x.contains("z"): return .orange
        case let x where x.contains("stop") || x.contains("t"): return .yellow
        default: return .secondary
        }
    }

    private func rssColor(_ bytes: UInt64) -> Color {
        let mb = Double(bytes) / 1024.0 / 1024.0
        if mb > 1024 { return .red }
        if mb > 512 { return .orange }
        if mb > 128 { return .yellow }
        return .primary
    }
}

/// Comparator for `Optional<UInt64>` columns (Table needs a non-optional type
/// for comparator injection). nil sorts last when ascending.
private struct OptionalIntComparator: SortComparator {
    var order: SortOrder = .forward

    func compare(_ lhs: UInt64?, _ rhs: UInt64?) -> ComparisonResult {
        switch (lhs, rhs) {
        case (nil, nil): return .orderedSame
        case (nil, _):   return order == .forward ? .orderedDescending : .orderedAscending
        case (_, nil):   return order == .forward ? .orderedAscending : .orderedDescending
        case let (l?, r?):
            if l == r { return .orderedSame }
            return order == .forward
                ? (l < r ? .orderedAscending : .orderedDescending)
                : (l < r ? .orderedDescending : .orderedAscending)
        }
    }

    var sortOrder: SortOrder {
        get { order }
        set { order = newValue }
    }
}
