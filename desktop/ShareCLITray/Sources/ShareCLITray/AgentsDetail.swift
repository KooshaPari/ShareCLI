/// AgentsDetail.swift — extracted agent detail panel from AgentsPage.
///
/// Contains: AgentsDetailView with agent detail views (header, fields, RSS
/// chart, cmdline, actions) and helper functions (stateColor, rssColor).
///
/// Part of closure B1 (house-limit decomposition).

import SwiftUI
import ShareCLICore

/// Detail panel for a selected agent — renders header, fields, RSS chart,
/// command-line, and kill action. Extracted from AgentsPage to meet the
/// 500-line house limit.
struct AgentsDetailView: View {
    let agent: AgentProcRow
    @ObservedObject var state: AppState

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                detailHeader
                detailFields
                detailRSSChart
                detailCmdline
                detailActions
            }
            .padding(16)
        }
        .background(.background)
    }

    // MARK: - Header

    private var detailHeader: some View {
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

    // MARK: - Fields

    private var detailFields: some View {
        VStack(alignment: .leading, spacing: 6) {
            detailRow("PID", value: "\(agent.pid)")
            detailRow("Family", value: agent.family)
            detailRow("Comm", value: agent.comm, mono: true)
            detailRow("State", value: agent.state, color: agentStateColor(agent.state))
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

    // MARK: - RSS Chart

    private var detailRSSChart: some View {
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
                    GeometryReader { geo in
                        let path = sparklinePath(
                            values: values,
                            in: CGRect(x: 0, y: 0, width: geo.size.width, height: 56)
                        )
                        path
                            .stroke(agentRSSColor(agent.mem_rss_bytes), lineWidth: 1.5)
                        path
                            .fill(LinearGradient(
                                colors: [
                                    agentRSSColor(agent.mem_rss_bytes).opacity(0.35),
                                    agentRSSColor(agent.mem_rss_bytes).opacity(0.0),
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
                let (lo, hi) = minMax(values)
                let y = rect.maxY - rect.height * CGFloat((v - lo) / max(1, hi - lo))
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

    // MARK: - Cmdline

    private var detailCmdline: some View {
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

    // MARK: - Actions

    private var detailActions: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                Task { await state.kill(pid: agent.pid) }
            } label: {
                Label("Kill PID \(agent.pid)", systemImage: "xmark.octagon.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(CTAButtonStyle.tint(for: .destructive))
        }
    }
}

// MARK: - Shared helpers (used by both AgentsPage table and AgentsDetailView)

/// State colour for agent process state badges. Shared between the agents
/// table (AgentsPage) and the detail panel (AgentsDetailView).
func agentStateColor(_ s: String) -> Color {
    switch s.lowercased() {
    case let x where x.contains("run"): return .green
    case let x where x.contains("sleep"): return .blue
    case let x where x.contains("idle"): return .gray
    case let x where x.contains("zombie") || x.contains("z"): return .orange
    case let x where x.contains("stop") || x.contains("t"): return .yellow
    default: return .secondary
    }
}

/// RSS colour for agent memory indicators. Shared between the agents
/// table and the detail panel.
func agentRSSColor(_ bytes: UInt64) -> Color {
    let mb = Double(bytes) / 1024.0 / 1024.0
    if mb > 1024 { return .red }
    if mb > 512 { return .orange }
    if mb > 128 { return .yellow }
    return .primary
}
