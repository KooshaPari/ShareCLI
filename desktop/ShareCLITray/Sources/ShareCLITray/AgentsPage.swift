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

    /// Per-row kill confirmation gate (closure batch A2 followup). The Table
    /// `Actions` column kill button routes through this gate rather than firing
    /// `state.kill(pid:)` directly, so the user always sees a confirmation
    /// dialog before a destructive action runs.
    @State private var killGate = DestructiveKillGate()

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
            Group {
                if let agent = selectedAgent {
                    AgentsDetailView(agent: agent, state: state)
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
            .frame(minWidth: 320, idealWidth: 360)
            .onChange(of: selectedPID) { _, newValue in
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
                Badge(text: agent.state, color: agentStateColor(agent.state))
            }
            .width(80)

            TableColumn("RSS", value: \.mem_rss_bytes) { agent in
                Text(agent.mem_rss)
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(agentRSSColor(agent.mem_rss_bytes))
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
                    killGate.request(.selected([agent.pid]))
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.red)
                }
                .buttonStyle(.borderless)
                .help("Kill PID \(agent.pid) (\(agent.comm))")
            }
            .width(40)
        }
        .confirmationDialog(
            killGate.pending?.confirmTitle ?? "Confirm kill",
            isPresented: Binding(
                get: { killGate.isConfirming },
                set: { if !$0 { killGate.decline() } }
            ),
            titleVisibility: .visible
        ) {
            if let scope = killGate.pending {
                Button(scope.confirmButtonLabel, role: .destructive) {
                    performKill(killGate.confirm())
                }
                Button(DestructiveKillGate.cancelLabel, role: .cancel) {
                    killGate.decline()
                }
            }
        } message: {
            Text(killGate.pending?.confirmMessage ?? "")
        }
    }

    /// Executes the kill for the given scope. `nil` (nothing confirmed) is a
    /// no-op so a per-row delete button can never fire without the dialog.
    private func performKill(_ scope: KillScope?) {
        guard let scope else { return }
        switch scope {
        case .selected(let pids):
            for pid in pids {
                Task { await state.kill(pid: pid) }
            }
        case .all:
            for agent in allAgents {
                Task { await state.kill(pid: agent.pid) }
            }
        }
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
