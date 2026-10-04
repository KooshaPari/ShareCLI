/// PoolPage.swift — dedicated page for the process pool (PR 3 of dashboard expansion plan).
///
/// Reuses `state.poolStatus: PoolSnapshot?` (already in `monitoring.report` —
/// no new IPC needed). Renders three panels:
///
///   ┌─────────────────┬─────────────────────────┐
///   │ Pool composition│ Issues panel            │
///   │  node / bun /   │  - each `issues[]` entry│
///   │  max / healthy  │  - color-coded severity │
///   │  idle gauges    │  - empty state          │
///   ├─────────────────┴─────────────────────────┤
///   │ Gate status panel (mirrors Health > Thermal)│
///   │  thermal pressure · gate decision ·        │
///   │  contention · detected agents              │
///   └────────────────────────────────────────────┘
///
/// Visual language mirrors AgentsPage / ProcessesPage (HSplitView,
/// summary strip, color-coded badges, GeometryReader bars).
///
/// Part of: plans/2026-07-25-tray-dashboard-expanded-v1.md §2.1 Page 3.

import SwiftUI
import ShareCLICore

struct PoolPage: View {
    @ObservedObject var state: AppState

    @AppStorage("pool.selectedView") private var viewRaw: String = Subpage.composition.rawValue
    @State private var view: Subpage = .composition
    @State private var didLoadView = false

    enum Subpage: String, CaseIterable, Identifiable {
        case composition = "composition"
        case issues = "issues"
        case gate = "gate"
        var id: String { rawValue }
        var label: String {
            switch self {
            case .composition: return "Composition"
            case .issues: return "Issues"
            case .gate: return "Gate"
            }
        }
    }

    var body: some View {
        if state.poolStatus == nil {
            EmptyStateView(
                icon: "rectangle.stack.badge.minus",
                title: "No pool snapshot yet",
                subtitle: "The sidecar hasn't reported a pool snapshot. The pool is normally populated within ~1s of the daemon starting — if it's blank for longer, the sharecli-ipc daemon may be down or the socket path is unreachable.",
                variant: .hero,
                primaryTitle: "Refresh now",
                primaryIcon: "arrow.clockwise",
                primaryAction: { Task { await state.refresh() } },
                secondaryTitle: "Pool docs",
                secondaryIcon: "book",
                secondaryAction: {
                    if let url = URL(string: "https://docs.sharecli.dev/pool") { NSWorkspace.shared.open(url) }
                }
            )
        } else {
            HSplitView {
                compositionPane
                    .frame(minWidth: 360, idealWidth: 460)
                issuesOrGatePane
                    .frame(minWidth: 320, idealWidth: 380)
            }
            .frame(minWidth: 720, minHeight: 460)
            .toolbar {
                ToolbarItem {
                    Picker("", selection: $view) {
                        ForEach(Subpage.allCases) { v in
                            Text(v.label).tag(v)
                        }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(width: 280)
                    .onChange(of: view) { _, newValue in
                        viewRaw = newValue.rawValue
                    }
                }
            }
            .onAppear {
                if !didLoadView {
                    view = Subpage(rawValue: viewRaw) ?? .composition
                    didLoadView = true
                }
            }
        }
    }

    // MARK: - Composition pane (always visible)

    private var compositionPane: some View {
        VStack(spacing: 0) {
            compositionSummaryStrip
            poolBarCharts
            capacityFootnotes
            Spacer()
        }
    }

    private var compositionSummaryStrip: some View {
        let pool = state.poolStatus
        let nodeActive = pool.map { $0.node_total - $0.node_idle } ?? 0
        let bunActive = pool.map { $0.bun_total - $0.bun_idle } ?? 0
        return HStack(spacing: 12) {
            MetricCell(title: "Healthy", value: pool?.healthy == true ? "yes" : "no",
                       sub: pool?.healthy == true ? "no issues" : "see Issues pane",
                       color: pool?.healthy == true ? .green : .red,
                       icon: pool?.healthy == true ? "checkmark.seal.fill" : "exclamationmark.triangle.fill")
            MetricCell(title: "Node", value: "\(pool?.node_total ?? 0)",
                       sub: "\(pool?.node_idle ?? 0) idle · \(nodeActive) active",
                       color: .blue, icon: "server.rack")
            MetricCell(title: "Bun", value: "\(pool?.bun_total ?? 0)",
                       sub: "\(pool?.bun_idle ?? 0) idle · \(bunActive) active",
                       color: .purple, icon: "shippingbox.fill")
            MetricCell(title: "Max/type", value: "\(pool?.max_per_type ?? 0)",
                       sub: "configured ceiling",
                       color: .orange, icon: "gauge.with.dots.needle.bottom.50percent")
        }
        .padding(12)
        .background(.quaternary.opacity(0.5))
    }

    private var poolBarCharts: some View {
        VStack(alignment: .leading, spacing: 14) {
            PoolGaugeRow(label: "Node processes",
                         active: (state.poolStatus?.node_total ?? 0) - (state.poolStatus?.node_idle ?? 0),
                         idle: state.poolStatus?.node_idle ?? 0,
                         max: state.poolStatus?.max_per_type ?? 1,
                         color: .blue)
            PoolGaugeRow(label: "Bun processes",
                         active: (state.poolStatus?.bun_total ?? 0) - (state.poolStatus?.bun_idle ?? 0),
                         idle: state.poolStatus?.bun_idle ?? 0,
                         max: state.poolStatus?.max_per_type ?? 1,
                         color: .purple)
        }
        .padding(16)
    }

    private var capacityFootnotes: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Capacity readouts")
                .font(.caption)
                .foregroundStyle(.secondary)
            Text("Each row shows active vs idle slots against `pool.max_per_type`. The bar fills left→right; the right edge marks the configured ceiling.")
                .font(.caption2)
                .foregroundStyle(.secondary)
            if !state.isConnected {
                FailedActionBanner(
                    title: RetryCopy.disconnectedTitle,
                    detail: state.lastError,
                    systemImage: "wifi.slash",
                    actionTitle: RetryCopy.disconnectedStartLabel,
                    actionSystemImage: "play.fill",
                    actionHint: RetryCopy.startHint,
                    action: {
                        Task {
                            await SidecarSupervisor.shared.ensureRunning()
                            await state.refresh()
                        }
                    }
                )
                .font(.caption2)
                .padding(.top, 4)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary.opacity(0.3))
    }

    // MARK: - Right pane (Issues or Gate)

    @ViewBuilder
    private var issuesOrGatePane: some View {
        switch view {
        case .composition: issuesPane  // default right pane when view = composition
        case .issues: issuesPane
        case .gate: gatePane
        }
    }

    private var issuesPane: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Pool issues")
                .font(.headline)
            if let pool = state.poolStatus {
                if pool.issues.isEmpty {
                    emptyIssues(pool: pool)
                } else {
                    ScrollView {
                        VStack(spacing: 6) {
                            ForEach(Array(pool.issues.enumerated()), id: \.offset) { idx, issue in
                                IssueRow(text: issue, severity: severityFor(issue))
                            }
                        }
                    }
                }
            } else {
                Text("Waiting for pool.status…")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 80)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.quaternary.opacity(0.3))
    }

    private func emptyIssues(pool: PoolSnapshot) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 28))
                .foregroundStyle(.green)
            Text("No issues reported")
                .font(.headline)
                .foregroundStyle(.green)
            Text("Pool is healthy: \(pool.node_total) node, \(pool.bun_total) bun, all within `max_per_type=\(pool.max_per_type)`.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 280)
        }
        .frame(maxWidth: .infinity, minHeight: 160)
        .padding(.vertical, 8)
    }

    private var gatePane: some View {
        let gate = state.health?.gate
        return ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                gateHeader(gate: gate)
                gateRow(label: "Thermal pressure", value: gate?.thermal_pressure ?? "—",
                        color: thermalColor(gate?.thermal_pressure ?? ""))
                gateRow(label: "Gate decision", value: gate?.gate_decision ?? "—",
                        color: decisionColor(gate?.gate_decision ?? ""))
                gateRow(label: "Contention", value: gate?.agent_contention ?? "—",
                        color: contentionColor(gate?.agent_contention ?? ""))
                gateRow(label: "Detected agents", value: gate.map { "\($0.detected_agents)" } ?? "—",
                        color: .primary)
                gateRow(label: "Agent RSS (bytes)", value: gate.map { String($0.agent_total_rss_bytes) } ?? "—",
                        color: .primary)
                gateRow(label: "Agent RSS (human)", value: gate.map { OperatorDisplay.formatBytesCompact($0.agent_total_rss_bytes) } ?? "—",
                        color: .orange)
            }
            .padding(16)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.quaternary.opacity(0.3))
    }

    private func gateHeader(gate: GateStatusSnapshot?) -> some View {
        let visual: TrayGateVisual = {
            if let g = gate, state.isConnected {
                return OperatorDisplay.resolveTrayGateVisual(gate: g, connected: true)
            }
            return OperatorDisplay.resolveTrayGateVisual(
                thermalPressure: "UNAVAILABLE",
                gateDecision: "UNAVAILABLE",
                connected: false
            )
        }()
        return HStack(spacing: 8) {
            Image(systemName: visual.swiftSymbolName).foregroundStyle(visual.swiftColor)
            Text("Gate status")
                .font(.headline)
            Spacer()
            Text(visual.badgeLabel)
                .font(.caption2.weight(.semibold))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .foregroundStyle(visual.swiftColor)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(visual.swiftColor, lineWidth: 1)
                )
        }
    }

    private func gateRow(label: String, value: String, color: Color) -> some View {
        HStack {
            Text(label).font(.caption).foregroundStyle(.secondary)
                .frame(width: 160, alignment: .leading)
            Text(value).font(.system(.body, design: .monospaced)).foregroundStyle(color)
            Spacer()
        }
    }

    // MARK: - Severity heuristics

    private func severityFor(_ issue: String) -> IssueRow.Severity {
        let lower = issue.lowercased()
        if lower.contains("error") || lower.contains("fail") || lower.contains("deny") {
            return .critical
        }
        if lower.contains("warn") || lower.contains("throttle") || lower.contains("limit") {
            return .warning
        }
        return .info
    }

    private func thermalColor(_ s: String) -> Color {
        switch s {
        case "GREEN": return .green
        case "YELLOW": return .orange
        case "RED": return .red
        default: return .secondary
        }
    }

    private func decisionColor(_ s: String) -> Color {
        switch s {
        case "ADMIT": return .green
        case "DENY": return .red
        case "THROTTLE": return .orange
        default: return .secondary
        }
    }

    private func contentionColor(_ s: String) -> Color {
        let l = s.lowercased()
        if l.contains("contend") || l.contains("high") { return .orange }
        if l.contains("low") || l == "calm" { return .green }
        return .secondary
    }
}

// MARK: - Helpers

/// Shared "summary cell" used by the Pool composition strip. Same visual
/// language as `ProcessesPage.summaryCard` and `AgentsPage.summaryCard` but
/// with an explicit icon slot.
struct MetricCell: View {
    let title: String
    let value: String
    let sub: String
    let color: Color
    let icon: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Image(systemName: icon).foregroundStyle(color)
                Text(title).font(.caption2).foregroundStyle(.secondary)
            }
            Text(value).font(.system(.title3, design: .monospaced)).bold().foregroundStyle(color)
                .lineLimit(1).minimumScaleFactor(0.7)
            Text(sub).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

/// Horizontal gauge showing active vs idle slots, capped at `max`.
struct PoolGaugeRow: View {
    let label: String
    let active: Int
    let idle: Int
    let max: Int
    let color: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(label).font(.caption).bold()
                Spacer()
                Text("\(active) active · \(idle) idle / \(max) max")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
            GeometryReader { geo in
                let total = active + idle
                let ceiling = Swift.max(max, 1)
                let activeFrac = Double(active) / Double(ceiling)
                let idleFrac = Double(idle) / Double(ceiling)
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 4).fill(.quaternary)
                    RoundedRectangle(cornerRadius: 4)
                        .fill(color.opacity(0.85))
                        .frame(width: Swift.max(0, geo.size.width * CGFloat(activeFrac)))
                    RoundedRectangle(cornerRadius: 4)
                        .fill(color.opacity(0.35))
                        .frame(width: Swift.max(0, geo.size.width * CGFloat(activeFrac + idleFrac)))
                    // Marker at max_per_type
                    Rectangle()
                        .fill(Color.primary.opacity(0.5))
                        .frame(width: 1)
                        .position(x: geo.size.width, y: geo.size.height / 2)
                }
                .overlay(
                    Text("\(total) used")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundStyle(.white.opacity(0.9))
                        .padding(.leading, 6)
                        .frame(maxWidth: .infinity, alignment: .leading),
                    alignment: .leading
                )
            }
            .frame(height: 18)
        }
    }
}

/// One row of the Issues pane.
struct IssueRow: View {
    enum Severity {
        case info, warning, critical
        var color: Color {
            switch self {
            case .info: return .secondary
            case .warning: return .orange
            case .critical: return .red
            }
        }
        var icon: String {
            switch self {
            case .info: return "info.circle"
            case .warning: return "exclamationmark.triangle"
            case .critical: return "xmark.octagon"
            }
        }
    }

    let text: String
    let severity: Severity

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: severity.icon).foregroundStyle(severity.color)
            Text(text)
                .font(.system(.body, design: .monospaced))
                .foregroundStyle(severity == .info ? .primary : severity.color)
            Spacer()
        }
        .padding(10)
        .background(.quaternary)
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(severity.color.opacity(0.4), lineWidth: severity == .info ? 0 : 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }
}