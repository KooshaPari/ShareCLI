/// HealthThermal.swift — extracted thermal gate subpage from HealthPage.
///
/// Contains: ThermalGateSubpage, ThermalGauge, GateDecisionLogPanel, GateDecisionRow.
///
/// Part of task 1.28 (Tray declarative file decomposition).

import SwiftUI
import ShareCLICore

// MARK: - Thermal gate subpage

struct ThermalGateSubpage: View {
    @ObservedObject var state: AppState

    @AppStorage("thermal.rssUnit") private var rssUnitRaw: String = RssUnit.mb.rawValue

    enum RssUnit: String {
        case mb, gb
    }

    private var rssUnit: RssUnit {
        RssUnit(rawValue: rssUnitRaw) ?? .mb
    }

    private var gate: GateStatusSnapshot? {
        state.health?.gate
    }

    private var gateVisual: TrayGateVisual {
        if let g = gate, state.isConnected {
            return OperatorDisplay.resolveTrayGateVisual(gate: g, connected: true)
        }
        return OperatorDisplay.resolveTrayGateVisual(
            thermalPressure: "UNAVAILABLE",
            gateDecision: "UNAVAILABLE",
            connected: false
        )
    }

    private var agentRSSDisplay: String {
        guard let g = gate else { return "—" }
        let bytes = g.agent_total_rss_bytes
        switch rssUnit {
        case .gb:
            return String(format: "%.2f GB", Double(bytes) / 1_073_741_824.0)
        case .mb:
            return String(format: "%.1f MB", Double(bytes) / 1_048_576.0)
        }
    }

    private var thermalPressureLevel: Int {
        // 0 = green/normal, 1 = yellow/warning, 2 = red/critical, 3 = unavailable
        guard let g = gate, state.isConnected else { return 3 }
        switch g.thermal_pressure {
        case "GREEN": return 0
        case "YELLOW": return 1
        case "RED": return 2
        default: return 3
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // 4 large cards
                HStack(spacing: 12) {
                    LargeCard(
                        title: "Detected agents",
                        value: "\(gate?.detected_agents ?? 0)",
                        sub: "agents currently tracked",
                        icon: "person.2.fill",
                        color: .blue
                    )
                    LargeCard(
                        title: "Total agent RSS",
                        value: agentRSSDisplay,
                        sub: rssUnit == .mb ? "tap to switch to GB" : "tap to switch to MB",
                        icon: "memorychip",
                        color: .orange
                    ) {
                        // Unit toggle in the top-right of the RSS card
                        Picker("", selection: Binding(
                            get: { rssUnitRaw },
                            set: { rssUnitRaw = $0 }
                        )) {
                            Text("MB").tag(RssUnit.mb.rawValue)
                            Text("GB").tag(RssUnit.gb.rawValue)
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .frame(width: 90)
                    }
                    LargeCard(
                        title: "Gate decision",
                        value: gate?.gate_decision ?? "—",
                        sub: "admission policy result",
                        icon: gateVisual.swiftSymbolName,
                        color: gateVisual.swiftColor
                    ) {
                        // Colored badge in top-right
                        Text(gateVisual.badgeLabel)
                            .font(.caption2.weight(.semibold))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .foregroundStyle(gateVisual.swiftColor)
                            .overlay(
                                RoundedRectangle(cornerRadius: 4)
                                    .stroke(gateVisual.swiftColor, lineWidth: 1)
                            )
                    }
                    LargeCard(
                        title: "Contention",
                        value: gate?.agent_contention ?? "—",
                        sub: "resource pressure state",
                        icon: contentionIcon,
                        color: contentionColor
                    )
                }

                // Thermal pressure gauge
                VStack(alignment: .leading, spacing: 8) {
                    Text("Thermal pressure")
                        .font(.headline)
                    ThermalGauge(level: thermalPressureLevel)
                    Text(thermalPressureLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(14)
                .background(.quaternary.opacity(0.5))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Last 20 gate decisions log
                GateDecisionLogPanel(state: state)

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
                    .font(.caption)
                    .padding(.top, 4)
                }
            }
            .padding(16)
        }
    }

    private var contentionIcon: String {
        switch (gate?.agent_contention ?? "").lowercased() {
        case let x where x.contains("contend") || x.contains("high"): return "exclamationmark.triangle.fill"
        case let x where x.contains("low") || x == "calm": return "checkmark.seal.fill"
        default: return "circle.dotted"
        }
    }

    private var contentionColor: Color {
        switch (gate?.agent_contention ?? "").lowercased() {
        case let x where x.contains("contend") || x.contains("high"): return .orange
        case let x where x.contains("low") || x == "calm": return .green
        default: return .secondary
        }
    }

    private var thermalPressureLabel: String {
        switch thermalPressureLevel {
        case 0: return "Nominal — within comfortable thermal envelope."
        case 1: return "Elevated — gate may begin throttling."
        case 2: return "Critical — gate expected to deny new spawns."
        default: return "Unavailable — sidecar not reporting thermal pressure."
        }
    }
}

/// Three-segment horizontal gauge for thermal pressure level.
/// Renders as a filled bar where the fill length and colour correspond to the
/// current pressure level (green / yellow / red / gray-unavailable).
struct ThermalGauge: View {
    let level: Int // 0=green, 1=yellow, 2=red, 3=unavailable

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 6)
                    .fill(.quaternary)
                RoundedRectangle(cornerRadius: 6)
                    .fill(fillColor)
                    .frame(width: max(0, geo.size.width * fillFrac))
                HStack(spacing: 0) {
                    ForEach(0..<4, id: \.self) { i in
                        Rectangle()
                            .fill(Color.black.opacity(i == level ? 0.18 : 0.04))
                            .frame(width: geo.size.width / 4)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .frame(height: 18)
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(fillColor.opacity(0.5), lineWidth: 1)
        )
    }

    private var fillColor: Color {
        switch level {
        case 0: return .green
        case 1: return .yellow
        case 2: return .red
        default: return .gray
        }
    }

    private var fillFrac: Double {
        switch level {
        case 0: return 0.25
        case 1: return 0.5
        case 2: return 0.85
        default: return 0.0
        }
    }
}

/// "Last 20 gate decisions" rolling log panel — the most novel element of
/// PR 6. Renders `AppState.gateDecisionHistory` newest-first with color-coded
/// rows.
struct GateDecisionLogPanel: View {
    @ObservedObject var state: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Last \(AppState.gateDecisionHistoryCap) gate decisions")
                    .font(.headline)
                Spacer()
                Text("\(state.gateDecisionHistory.count) sample(s) buffered")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if state.gateDecisionHistory.isEmpty {
                emptyLog
            } else {
                VStack(spacing: 2) {
                    ForEach(Array(state.gateDecisionHistory.reversed().enumerated()), id: \.element.id) { idx, sample in
                        GateDecisionRow(sample: sample, isLatest: idx == 0)
                    }
                }
                .padding(8)
                .background(.quaternary.opacity(0.3))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .padding(14)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var emptyLog: some View {
        VStack(spacing: 6) {
            Image(systemName: "tray")
                .font(.system(size: 28))
                .foregroundStyle(.secondary)
            Text("Awaiting first poll — gate decisions appear here as soon as the sidecar responds.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, minHeight: 80)
        .padding(.vertical, 8)
    }
}

struct GateDecisionRow: View {
    let sample: GateDecisionSample
    let isLatest: Bool

    private static let timeFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f
    }()

    var body: some View {
        HStack(spacing: 8) {
            Text(Self.timeFormatter.string(from: sample.timestamp))
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .leading)

            Text(sample.gateDecision)
                .font(.system(.caption, design: .monospaced).weight(isLatest ? .bold : .regular))
                .foregroundStyle(decisionColor)
                .frame(width: 86, alignment: .leading)

            Text("thermal \(sample.thermalPressure)")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(thermalColor)
                .frame(width: 116, alignment: .leading)

            Text("agents \(sample.detectedAgents)")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.primary)
                .frame(width: 64, alignment: .leading)

            Text("rss \(OperatorDisplay.formatBytesCompact(sample.agentTotalRssBytes))")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)

            Spacer(minLength: 8)

            Text(sample.agentContention)
                .font(.caption2.weight(.semibold))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .foregroundStyle(contentionColor)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(contentionColor, lineWidth: 1)
                )
        }
        .padding(.vertical, 3)
        .padding(.horizontal, 6)
        .background(isLatest ? Color.accentColor.opacity(0.08) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: 4))
    }

    private var decisionColor: Color {
        switch sample.gateDecision {
        case "ADMIT": return .green
        case "DENY": return .red
        case "THROTTLE": return .orange
        default: return .secondary
        }
    }

    private var thermalColor: Color {
        switch sample.thermalPressure {
        case "GREEN": return .green
        case "YELLOW": return .orange
        case "RED": return .red
        default: return .secondary
        }
    }

    private var contentionColor: Color {
        let c = sample.agentContention.lowercased()
        if c.contains("contend") || c.contains("high") { return .orange }
        if c.contains("low") || c == "calm" { return .green }
        return .secondary
    }
}

